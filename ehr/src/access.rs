//! Roles and what they allow (docs/ideas/ACCESS-ROLES.md §4, §5; migration 0011).
//!
//! A role is a set of **levels**, one per **lever** (None < View < Edit). A person holds any
//! number of roles; on each lever they have the highest level of their live roles (the owner's
//! fork 2A, 2026-11-15). Deny by default: a person with no role has None everywhere.
//!
//! The roles are a settings service (`roles`, versioned like the others); who holds which is
//! `user_roles`, where a row is never deleted. ehr reads a person's levels itself on every staff
//! request (`actor`): it never takes levels from the agent.
//!
//! The rules (§5), each refused here with a fixed line that names no person:
//! - **No one changes their own roles.**
//! - **You change only people at or below you:** to grant, revoke, remove, or edit a card, your
//!   level must be at least theirs on every lever, after the change.
//! - **The conflict rule:** a person who may read the audit logs and may not edit roles (the
//!   Compliance shape) may not also hold a role that edits roles or people.
//! - **The last holder:** a revoke or a removal that leaves no live person with a Team role that
//!   edits roles is refused, so the console can never be locked.

use std::collections::BTreeMap;

use serde_json::Value;
use sqlx::{PgConnection, PgPool, Row};

/// One thing a role can be allowed or refused (§3). The words are the store's and the wire's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lever {
    Phi,
    Auth,
    Roles,
    People,
    Agent,
    Vendors,
    Rules,
    Testing,
    Analytics,
    Flags,
    AuditLogs,
}

pub const LEVERS: [Lever; 11] = [
    Lever::Phi,
    Lever::Auth,
    Lever::Roles,
    Lever::People,
    Lever::Agent,
    Lever::Vendors,
    Lever::Rules,
    Lever::Testing,
    Lever::Analytics,
    Lever::Flags,
    Lever::AuditLogs,
];

impl Lever {
    pub fn word(self) -> &'static str {
        match self {
            Lever::Phi => "phi",
            Lever::Auth => "auth",
            Lever::Roles => "roles",
            Lever::People => "people",
            Lever::Agent => "agent",
            Lever::Vendors => "vendors",
            Lever::Rules => "rules",
            Lever::Testing => "testing",
            Lever::Analytics => "analytics",
            Lever::Flags => "flags",
            Lever::AuditLogs => "audit_logs",
        }
    }

    pub fn from_word(word: &str) -> Option<Lever> {
        LEVERS.into_iter().find(|l| l.word() == word)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Level {
    #[default]
    None,
    View,
    Edit,
}

impl Level {
    pub fn word(self) -> &'static str {
        match self {
            Level::None => "none",
            Level::View => "view",
            Level::Edit => "edit",
        }
    }

    pub fn from_word(word: &str) -> Option<Level> {
        match word {
            "none" => Some(Level::None),
            "view" => Some(Level::View),
            "edit" => Some(Level::Edit),
            _ => None,
        }
    }
}

/// Whose a role is: the hospital's, or ours (DeetsHealth Team; STAFF-SIGN-IN.md M6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    Hospital,
    Team,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub domain: Domain,
    pub levels: Levels,
}

/// A level on every lever.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Levels(BTreeMap<Lever, Level>);

impl Levels {
    /// The highest level on each lever across `roles`. No roles: None everywhere.
    pub fn of<'a>(roles: impl IntoIterator<Item = &'a Role>) -> Levels {
        let mut out = BTreeMap::new();
        for role in roles {
            for (lever, level) in &role.levels.0 {
                let at = out.entry(*lever).or_insert(Level::None);
                *at = (*at).max(*level);
            }
        }
        Levels(out)
    }

    pub fn get(&self, lever: Lever) -> Level {
        self.0.get(&lever).copied().unwrap_or_default()
    }

    pub fn may(&self, lever: Lever, level: Level) -> bool {
        self.get(lever) >= level
    }

    /// Whether these levels are at least `other`'s on every lever.
    pub fn covers(&self, other: &Levels) -> bool {
        LEVERS.into_iter().all(|l| self.get(l) >= other.get(l))
    }

    /// Each lever and its level, as words.
    pub fn words(&self) -> Vec<(&'static str, &'static str)> {
        LEVERS.into_iter().map(|l| (l.word(), self.get(l).word())).collect()
    }
}

/// The most roles a hospital's list holds, and the longest name.
pub const MAX_ROLES: usize = 50;
pub const MAX_ROLE_NAME: usize = 40;

/// A role id: a lower-case word, as `user_roles.role_id` checks it.
pub fn role_id_ok(id: &str) -> bool {
    let mut chars = id.chars();
    (1..=32).contains(&id.len())
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The roles list as the settings store holds it, checked: at most `MAX_ROLES`, each with a
/// unique id and name, a domain, and a level for every lever (no other key). At least one Team
/// role edits roles, so the console can always be managed. The error names a role's place in the
/// list, never its name.
pub fn roles_from(value: &Value) -> Result<Vec<Role>, String> {
    let list = value.as_array().ok_or("roles: a list")?;
    if list.is_empty() || list.len() > MAX_ROLES {
        return Err(format!("roles: 1 to {MAX_ROLES}"));
    }
    let mut out: Vec<Role> = Vec::with_capacity(list.len());
    for (i, item) in list.iter().enumerate() {
        let at = i + 1;
        let o = item.as_object().ok_or_else(|| format!("roles: role {at} is not an object"))?;
        if o.keys().any(|k| !matches!(k.as_str(), "id" | "name" | "domain" | "levels")) {
            return Err(format!("roles: role {at} has a key that is not id, name, domain or levels"));
        }
        let id = o.get("id").and_then(Value::as_str).filter(|id| role_id_ok(id)).ok_or_else(|| {
            format!("roles: role {at}'s id is not a lower-case word of at most 32 characters")
        })?;
        let name = o
            .get("name")
            .and_then(Value::as_str)
            .map(|n| n.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|n| !n.is_empty() && n.chars().count() <= MAX_ROLE_NAME && !n.chars().any(char::is_control))
            .ok_or_else(|| format!("roles: role {at}'s name is not 1 to {MAX_ROLE_NAME} characters on one line"))?;
        let domain = match o.get("domain").and_then(Value::as_str) {
            Some("hospital") => Domain::Hospital,
            Some("team") => Domain::Team,
            _ => return Err(format!("roles: role {at}'s domain is not hospital or team")),
        };
        let given = o.get("levels").and_then(Value::as_object).ok_or_else(|| format!("roles: role {at} has no levels"))?;
        let mut levels = BTreeMap::new();
        for (word, level) in given {
            let lever = Lever::from_word(word).ok_or_else(|| format!("roles: role {at} names a lever that does not exist"))?;
            let level = level
                .as_str()
                .and_then(Level::from_word)
                .ok_or_else(|| format!("roles: role {at}'s {} is not none, view or edit", lever.word()))?;
            levels.insert(lever, level);
        }
        if let Some(missing) = LEVERS.into_iter().find(|l| !levels.contains_key(l)) {
            return Err(format!("roles: role {at} has no level for {}", missing.word()));
        }
        if out.iter().any(|r| r.id == id) {
            return Err(format!("roles: role {at}'s id is used twice"));
        }
        if out.iter().any(|r| r.name.eq_ignore_ascii_case(&name)) {
            return Err(format!("roles: role {at}'s name is used twice"));
        }
        out.push(Role { id: id.to_string(), name, domain, levels: Levels(levels) });
    }
    if !out.iter().any(manages) {
        return Err("roles: no Team role edits roles, so no one could manage the console".to_string());
    }
    Ok(out)
}

/// A role that keeps the console manageable: a Team role with Edit on Roles.
fn manages(role: &Role) -> bool {
    role.domain == Domain::Team && role.levels.may(Lever::Roles, Level::Edit)
}

/// The conflict rule (§5): one person may not both audit and change access. Refused when the
/// roles together read the audit logs through a role with no Edit on Roles (the Compliance
/// shape), and another of them edits Roles or People.
pub fn conflict<'a>(roles: impl IntoIterator<Item = &'a Role> + Clone) -> Option<&'static str> {
    let auditor = roles
        .clone()
        .into_iter()
        .any(|r| r.levels.may(Lever::AuditLogs, Level::View) && !r.levels.may(Lever::Roles, Level::Edit));
    let changer = roles
        .into_iter()
        .any(|r| r.levels.may(Lever::Roles, Level::Edit) || r.levels.may(Lever::People, Level::Edit));
    (auditor && changer).then_some("a person who reviews the audit logs may not also hold a role that changes roles or people")
}

/// Why an access change was refused: a fixed line, never a name.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// The actor lacks this lever at this level.
    Needs(Lever, Level),
    /// One of the rules above.
    Rule(&'static str),
    NotFound(&'static str),
    Store(String),
}

/// The active roles version: its id and its roles.
pub async fn active_roles(conn: &mut PgConnection) -> Result<(i64, Vec<Role>), String> {
    let row = sqlx::query(
        "SELECT a.version_id, r.value::text AS value FROM setting_activations a
         JOIN settings_roles r ON r.version_id = a.version_id AND r.key = 'roles'
         WHERE a.service = 'roles' ORDER BY a.id DESC LIMIT 1",
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(|_| "access: read the roles".to_string())?
    .ok_or("access: no roles version")?;
    let value: Value = serde_json::from_str(&row.get::<String, _>("value")).map_err(|_| "access: the roles are not JSON")?;
    Ok((row.get("version_id"), roles_from(&value)?))
}

/// A person as a request names them: their id, their live roles and the levels those give.
#[derive(Clone, Debug, PartialEq)]
pub struct Actor {
    pub id: String,
    /// Their live roles that the active version still has, by id.
    pub role_ids: Vec<String>,
    pub roles_version: i64,
    pub levels: Levels,
}

/// A person's live role ids (granted, not revoked).
async fn held(conn: &mut PgConnection, user_id: &str) -> Result<Vec<String>, String> {
    let rows = sqlx::query("SELECT role_id FROM user_roles WHERE user_id = $1::uuid AND revoked_at IS NULL ORDER BY role_id")
        .bind(user_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(|_| "access: read the grants".to_string())?;
    Ok(rows.iter().map(|r| r.get("role_id")).collect())
}

/// Whether this person is on the list now.
async fn live(conn: &mut PgConnection, user_id: &str) -> Result<bool, String> {
    Ok(sqlx::query("SELECT 1 AS one FROM users WHERE id = $1::uuid AND removed_at IS NULL")
        .bind(user_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| "access: find the person".to_string())?
        .is_some())
}

/// The person `user_id` as an actor: None when they are not on the list now (removed, or never
/// there). A role id the active version no longer has gives nothing.
pub async fn actor(conn: &mut PgConnection, user_id: &str) -> Result<Option<Actor>, String> {
    if !uuid_ok(user_id) || !live(conn, user_id).await? {
        return Ok(None);
    }
    let (version, roles) = active_roles(conn).await?;
    let ids = held(conn, user_id).await?;
    let mine: Vec<&Role> = roles.iter().filter(|r| ids.contains(&r.id)).collect();
    Ok(Some(Actor {
        id: user_id.to_string(),
        role_ids: mine.iter().map(|r| r.id.clone()).collect(),
        roles_version: version,
        levels: Levels::of(mine.iter().copied()),
    }))
}

/// The pool form of `actor`.
pub async fn actor_in(pool: &PgPool, user_id: &str) -> Result<Option<Actor>, String> {
    let mut conn = pool.acquire().await.map_err(|_| "access: connect".to_string())?;
    actor(&mut conn, user_id).await
}

/// A person's live roles, by id, for the People list: every live person at once.
pub async fn all_held(pool: &PgPool) -> Result<BTreeMap<String, Vec<String>>, String> {
    let rows = sqlx::query("SELECT user_id::text AS user_id, role_id FROM user_roles WHERE revoked_at IS NULL ORDER BY role_id")
        .fetch_all(pool)
        .await
        .map_err(|_| "access: read the grants".to_string())?;
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in rows {
        out.entry(r.get("user_id")).or_default().push(r.get("role_id"));
    }
    Ok(out)
}

/// The levels `role_ids` give under `roles`.
fn levels_of(roles: &[Role], role_ids: &[String]) -> Levels {
    Levels::of(roles.iter().filter(|r| role_ids.contains(&r.id)))
}

/// Whether, with `user_id`'s grants as they would be after a change, some live person still
/// holds a role that manages the console (the last-holder rule). `without`: a person who would
/// be removed; `revoke`: a grant that would end.
async fn still_managed(
    conn: &mut PgConnection,
    roles: &[Role],
    without: Option<&str>,
    revoke: Option<(&str, &str)>,
) -> Result<bool, String> {
    let managing: Vec<&str> = roles.iter().filter(|r| manages(r)).map(|r| r.id.as_str()).collect();
    let rows = sqlx::query(
        "SELECT g.user_id::text AS user_id, g.role_id FROM user_roles g JOIN users u ON u.id = g.user_id
         WHERE g.revoked_at IS NULL AND u.removed_at IS NULL",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|_| "access: read the grants".to_string())?;
    Ok(rows.iter().any(|r| {
        let user: String = r.get("user_id");
        let role: String = r.get("role_id");
        managing.contains(&role.as_str())
            && Some(user.as_str()) != without
            && revoke.is_none_or(|(u, ro)| !(u == user && ro == role))
    }))
}

/// The checks a change to another person's access or card runs (§5): the lever, not oneself
/// (for roles), and at or below the actor before and after. `after`: the person's role ids after
/// the change. Returns the person's role ids now.
async fn may_change(
    conn: &mut PgConnection,
    who: &Actor,
    roles: &[Role],
    user_id: &str,
    after: Option<&[String]>,
) -> Result<Vec<String>, Refused> {
    if !uuid_ok(user_id) || !live(conn, user_id).await.map_err(Refused::Store)? {
        return Err(Refused::NotFound("no such person on the list"));
    }
    let now = held(conn, user_id).await.map_err(Refused::Store)?;
    if !who.levels.covers(&levels_of(roles, &now)) {
        return Err(Refused::Rule("you may change only people whose access is at or below yours"));
    }
    if let Some(after) = after {
        if !who.levels.covers(&levels_of(roles, after)) {
            return Err(Refused::Rule("you may not give more access than you have"));
        }
    }
    Ok(now)
}

/// Grants `role_id` to `user_id`, in `conn`'s transaction. The caller writes the audit row in
/// the same transaction.
pub async fn grant(conn: &mut PgConnection, who: &Actor, user_id: &str, role_id: &str) -> Result<(), Refused> {
    if !who.levels.may(Lever::Roles, Level::Edit) {
        return Err(Refused::Needs(Lever::Roles, Level::Edit));
    }
    if who.id == user_id {
        return Err(Refused::Rule("no one changes their own roles"));
    }
    let (_, roles) = active_roles(conn).await.map_err(Refused::Store)?;
    if !roles.iter().any(|r| r.id == role_id) {
        return Err(Refused::NotFound("no such role"));
    }
    let mut after = held(conn, user_id).await.map_err(Refused::Store)?;
    if after.iter().any(|r| r == role_id) {
        return Err(Refused::Rule("they hold that role already"));
    }
    after.push(role_id.to_string());
    may_change(conn, who, &roles, user_id, Some(&after)).await?;
    if let Some(why) = conflict(roles.iter().filter(|r| after.contains(&r.id))) {
        return Err(Refused::Rule(why));
    }
    sqlx::query("INSERT INTO user_roles (user_id, role_id, granted_by) VALUES ($1::uuid, $2, $3::uuid)")
        .bind(user_id)
        .bind(role_id)
        .bind(&who.id)
        .execute(&mut *conn)
        .await
        .map_err(|_| Refused::Store("access: grant".to_string()))?;
    Ok(())
}

/// Revokes `user_id`'s live `role_id`, in `conn`'s transaction.
pub async fn revoke(conn: &mut PgConnection, who: &Actor, user_id: &str, role_id: &str) -> Result<(), Refused> {
    if !who.levels.may(Lever::Roles, Level::Edit) {
        return Err(Refused::Needs(Lever::Roles, Level::Edit));
    }
    if who.id == user_id {
        return Err(Refused::Rule("no one changes their own roles"));
    }
    let (_, roles) = active_roles(conn).await.map_err(Refused::Store)?;
    let now = may_change(conn, who, &roles, user_id, None).await?;
    if !now.iter().any(|r| r == role_id) {
        return Err(Refused::NotFound("they do not hold that role"));
    }
    if !still_managed(conn, &roles, None, Some((user_id, role_id))).await.map_err(Refused::Store)? {
        return Err(Refused::Rule("the last person who can manage the console keeps that role"));
    }
    sqlx::query(
        "UPDATE user_roles SET revoked_at = now(), revoked_by = $3::uuid
         WHERE user_id = $1::uuid AND role_id = $2 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(role_id)
    .bind(&who.id)
    .execute(&mut *conn)
    .await
    .map_err(|_| Refused::Store("access: revoke".to_string()))?;
    Ok(())
}

/// The checks before a person is removed: People Edit, at or below the actor, and not the last
/// person who can manage the console.
pub async fn may_remove(conn: &mut PgConnection, who: &Actor, user_id: &str) -> Result<(), Refused> {
    if !who.levels.may(Lever::People, Level::Edit) {
        return Err(Refused::Needs(Lever::People, Level::Edit));
    }
    let (_, roles) = active_roles(conn).await.map_err(Refused::Store)?;
    may_change(conn, who, &roles, user_id, None).await?;
    if !still_managed(conn, &roles, Some(user_id), None).await.map_err(Refused::Store)? {
        return Err(Refused::Rule("the last person who can manage the console cannot be removed"));
    }
    Ok(())
}

/// The checks before a person's card or picture changes: People Edit, and at or below the actor
/// (one's own card included: everyone is at or below themself).
pub async fn may_edit_card(conn: &mut PgConnection, who: &Actor, user_id: &str) -> Result<(), Refused> {
    if !who.levels.may(Lever::People, Level::Edit) {
        return Err(Refused::Needs(Lever::People, Level::Edit));
    }
    let (_, roles) = active_roles(conn).await.map_err(Refused::Store)?;
    may_change(conn, who, &roles, user_id, None).await.map(|_| ())
}

/// A role save keeps the console manageable and every live person's grants allowed: every role
/// still held stays in the list (a role in use is revoked first), and no person ends up in a
/// conflict. `roles`: the new list, already checked by `roles_from`.
pub async fn may_save_roles(conn: &mut PgConnection, roles: &[Role]) -> Result<(), String> {
    let rows = sqlx::query(
        "SELECT g.user_id::text AS user_id, g.role_id FROM user_roles g JOIN users u ON u.id = g.user_id
         WHERE g.revoked_at IS NULL AND u.removed_at IS NULL",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|_| "access: read the grants".to_string())?;
    let mut by_person: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in &rows {
        by_person.entry(r.get("user_id")).or_default().push(r.get("role_id"));
    }
    for ids in by_person.values() {
        if ids.iter().any(|id| !roles.iter().any(|r| &r.id == id)) {
            return Err("roles: a role that someone holds cannot be removed; revoke it first".to_string());
        }
        if conflict(roles.iter().filter(|r| ids.contains(&r.id))).is_some() {
            return Err("roles: the change would give someone both audit and access-changing roles".to_string());
        }
    }
    let managed = by_person.values().any(|ids| roles.iter().any(|r| manages(r) && ids.contains(&r.id)));
    if !managed {
        return Err("roles: no one would be left who can manage the console".to_string());
    }
    Ok(())
}

/// Gives the owner's row Deets when it has no live role (users::seed, the first person on an
/// empty list). Granted by the system.
pub async fn grant_owner(conn: &mut PgConnection, user_id: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO user_roles (user_id, role_id) SELECT $1::uuid, 'deets'
         WHERE NOT EXISTS (SELECT 1 FROM user_roles WHERE user_id = $1::uuid AND revoked_at IS NULL)",
    )
    .bind(user_id)
    .execute(&mut *conn)
    .await
    .map_err(|_| "access: grant the owner".to_string())?;
    Ok(())
}

/// A uuid in its text form.
pub fn uuid_ok(s: &str) -> bool {
    s.len() == 36 && s.char_indices().all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { c == '-' } else { c.is_ascii_hexdigit() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The Default roles, as migration 0011 seeds them.
    fn defaults() -> Vec<Role> {
        let sql = include_str!("../migrations/0011_roles.sql");
        let start = sql.find("$json$").unwrap() + "$json$".len();
        let end = start + sql[start..].find("$json$").unwrap();
        roles_from(&serde_json::from_str(&sql[start..end]).unwrap()).unwrap()
    }

    fn role(roles: &[Role], id: &str) -> Role {
        roles.iter().find(|r| r.id == id).unwrap().clone()
    }

    #[test]
    fn the_default_roles_are_the_owner_s_templates() {
        let roles = defaults();
        let ids: Vec<&str> = roles.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            ids,
            ["deets", "director", "manager", "senior_analyst", "junior_analyst", "clinician", "compliance", "engineer", "intern"]
        );
        let deets = role(&roles, "deets");
        assert_eq!(deets.domain, Domain::Team);
        // Edit on every lever, PHI too (the owner, 2026-11-18), so Deets covers every template.
        assert!(LEVERS.iter().all(|l| deets.levels.get(*l) == Level::Edit));
        assert!(roles.iter().all(|r| deets.levels.covers(&r.levels)));
        assert_eq!(role(&roles, "junior_analyst").levels.get(Lever::Agent), Level::View);
    }

    #[test]
    fn several_roles_give_the_highest_level_on_each_lever() {
        let roles = defaults();
        let both = Levels::of([&role(&roles, "clinician"), &role(&roles, "junior_analyst")]);
        assert_eq!(both.get(Lever::Phi), Level::View);
        assert_eq!(both.get(Lever::Flags), Level::Edit);
        assert_eq!(both.get(Lever::Agent), Level::View);
        assert_eq!(both.get(Lever::Roles), Level::None);
        assert!(Levels::of([]).words().iter().all(|(_, l)| *l == "none"), "no role: None everywhere");
    }

    #[test]
    fn covers_is_at_least_on_every_lever() {
        let roles = defaults();
        let deets = role(&roles, "deets").levels;
        assert!(deets.covers(&role(&roles, "junior_analyst").levels));
        assert!(deets.covers(&role(&roles, "director").levels));
        let engineer = role(&roles, "engineer").levels;
        assert!(!engineer.covers(&role(&roles, "clinician").levels), "Clinician has PHI; Engineer has none");
        assert!(deets.covers(&deets));
    }

    #[test]
    fn compliance_cannot_also_change_access() {
        let roles = defaults();
        assert!(conflict([&role(&roles, "compliance"), &role(&roles, "manager")]).is_some());
        assert!(conflict([&role(&roles, "compliance"), &role(&roles, "clinician")]).is_none());
        assert!(conflict([&role(&roles, "director")]).is_none(), "one role that does both is the hospital's choice");
    }

    #[test]
    fn a_roles_list_is_checked_whole() {
        let lv = |level: &str| LEVERS.iter().map(|l| (l.word().to_string(), json!(level))).collect::<serde_json::Map<_, _>>();
        let team = json!({"id": "deets", "name": "Deets", "domain": "team", "levels": lv("edit")});
        assert!(roles_from(&json!([team.clone()])).is_ok());
        let hospital_only = json!([{"id": "a", "name": "A", "domain": "hospital", "levels": lv("edit")}]);
        assert!(roles_from(&hospital_only).unwrap_err().contains("no Team role"));
        let mut missing = lv("view");
        missing.remove("flags");
        let bad = json!([team.clone(), {"id": "b", "name": "B", "domain": "hospital", "levels": missing}]);
        assert!(roles_from(&bad).unwrap_err().contains("role 2 has no level for flags"));
        let twice = json!([team.clone(), {"id": "c", "name": "deets", "domain": "hospital", "levels": lv("none")}]);
        assert!(roles_from(&twice).unwrap_err().contains("name is used twice"));
        let bad_id = json!([team.clone(), {"id": "Bad Id", "name": "X", "domain": "hospital", "levels": lv("none")}]);
        assert!(roles_from(&bad_id).unwrap_err().contains("role 2's id"));
        let bad_level = json!([team, {"id": "d", "name": "D", "domain": "hospital", "levels": {"phi": "all"}}]);
        assert!(roles_from(&bad_level).is_err());
    }

    #[test]
    fn levers_and_levels_round_trip_their_words() {
        for l in LEVERS {
            assert_eq!(Lever::from_word(l.word()), Some(l));
        }
        for l in [Level::None, Level::View, Level::Edit] {
            assert_eq!(Level::from_word(l.word()), Some(l));
        }
        assert!(Level::None < Level::View && Level::View < Level::Edit);
        assert!(role_id_ok("senior_analyst") && !role_id_ok("1x") && !role_id_ok("") && !role_id_ok(&"a".repeat(33)));
    }
}
