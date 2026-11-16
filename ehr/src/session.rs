//! Each call's session, named by an opaque handle (EHR-SERVICE.md §1).
//!
//! The map of sessions has one lock; each session has its own, so a slow booking on one call
//! never holds up another. Nothing here has `Debug`: a handle is a credential, and a session
//! holds the verified patient.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use deetshealth_core::Shielded;
use tokio::sync::Mutex;
use zeroize::Zeroize;

use crate::{
    hospital::Hospital,
    scheduler::{BookResult, SlotId},
    visit_about::BookNote,
};

/// The most sessions held at once. A flood of OpenCall cannot grow memory without end.
const MAX_SESSIONS: usize = 10_000;
/// A session the agent never closed is wiped after this long. No phone call lasts it.
const MAX_AGE: Duration = Duration::from_secs(3 * 60 * 60);

/// 128 random bits from the OS: a call's handle, or a slot token. Wiped on drop.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Token([u8; 16]);

impl Token {
    pub fn random() -> Self {
        let mut b = [0u8; 16];
        getrandom::fill(&mut b).expect("the OS random source");
        Self(b)
    }

    /// 32 lowercase hex characters, or nothing.
    pub fn parse(s: &str) -> Option<Self> {
        if s.len() != 32 || !s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) {
            return None;
        }
        let mut b = [0u8; 16];
        for (i, byte) in b.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Self(b))
    }

    pub(crate) fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// The verified patient, as the EHR knows them. Set only by verification, never from the wire.
/// Compares without revealing (`Shielded`'s comparison).
#[derive(Clone, PartialEq, Eq)]
pub struct PatientKey(pub(crate) Shielded<String>);

/// How far the caller has come.
pub enum Stage {
    /// Both factors have not passed. Chart and booking calls are refused. `Some` once a name
    /// and date of birth matched one patient and a code went out.
    Unverified(Option<Pending>),
    /// Both factors passed (SECURITY.md §4.4).
    Verified(PatientKey),
    /// Too many wrong codes: the chart path is closed for this call. Offer a person.
    Locked,
}

/// The patient a name and date of birth matched, and the code sent to them.
pub struct Pending {
    pub patient: PatientKey,
    /// The patient's demographics for a slot search (verify.rs `Candidate::facts`).
    pub facts: Option<Shielded<String>>,
    /// The patient's PCPs on file (verify.rs `Candidate::pcp`).
    pub pcp: Vec<crate::hospital::ClinicianRef>,
    pub phone: Shielded<String>,
    pub code: Shielded<String>,
    pub issued: Instant,
}

/// Which EHR a call reads (OpenCall's `ehr`). Set once, when the call opens.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// ehr's own fakes (`ehr/fixtures/sandbox.json`).
    Local,
    /// Epic's public sandbox (epic_ehr.rs). Dev only.
    EpicSandbox,
}

impl Source {
    /// Whether the source holds only test data (DATABASE.md §7.5). A synthetic call may reach
    /// only a sandbox, a real call only a production EHR. A new source must answer here.
    pub fn is_sandbox(self) -> bool {
        match self {
            Source::Local | Source::EpicSandbox => true,
        }
    }
}

pub struct Session {
    /// The EHR this call reads.
    pub source: Source,
    pub stage: Stage,
    /// How many times the caller gave a name and date of birth.
    pub identifies: u32,
    /// How many times a code was sent again.
    pub resends: u32,
    /// How many wrong codes.
    pub code_failures: u32,
    /// The slots of the last FindSlots, by the token the agent got for each.
    pub offered: HashMap<Token, SlotId>,
    /// The rest of the last search's slots, not offered yet, and the EHR's links to more pages
    /// (FindSlots with `more`; EPIC.md §7.10).
    pub rest: Vec<crate::scheduler::Slot>,
    pub next_pages: Vec<String>,
    /// The verified patient's demographics, for a slot search. Set when the code passes.
    pub facts: Option<Shielded<String>>,
    /// The last FindSlots's reason, screened: Book sends it to the scheduler as the visit's
    /// note. Lives one call long, like the rest of the session.
    pub note: Option<BookNote>,
    /// The text message's words for this call, from its prompt version (OpenCall's
    /// `prompt_version`); None: the prompt file compiled into ehr.
    pub text_template: Option<String>,
    /// The hospital this call runs with (HOSPITAL-CONFIG.md): the active version when the call
    /// opened, or the file. A save during the call does not change it.
    pub hospital: Arc<Hospital>,
    /// Each booking's answer, by its idempotency key, with the slot token it was for.
    pub booked: HashMap<String, (Token, BookResult)>,
    /// The EHR ledger (ledger.rs): each request ehr sent the EHR on this call, by the gRPC call
    /// that sent it. Names and counts only. CloseCall hands it to the agent.
    pub ledger: Vec<(&'static str, crate::ledger::Entry)>,
    /// Where the verified caller was seen in the hospital's window (`established_years`), read
    /// once, at the first FindSlots that needs it (BOOKING-RULES.md §3.4). None: not read yet.
    /// Lives one call long, like the rest of the session.
    pub seen: Option<Vec<crate::scheduler::SeenVisit>>,
    /// In person, online or either, as the last FindSlots asked: its next pages keep to it.
    pub mode: crate::scheduler::VisitMode,
    /// The verified patient's PCPs on file (BOOKING-RULES.md §3.2). Set when the code passes.
    pub pcp: Vec<crate::hospital::ClinicianRef>,
    /// The last search's PCP (the one the caller saw last), its place, and the slot offered as
    /// sooner with another clinician (P-A): each slot of that search, on any page, says so.
    pub search_pcp: Option<crate::hospital::ClinicianRef>,
    pub sooner: Option<crate::scheduler::SlotId>,
    /// The last search's usual place (§3.2), and its words for the caller: "more" keeps to it.
    pub search_place: Option<crate::hospital::PlaceRef>,
    pub place_words: Option<Shielded<String>>,
    opened: Instant,
}

/// Why a session cannot be used. Each maps to one gRPC code (service.rs).
pub enum Refusal {
    /// No session has this handle: never opened, closed or expired.
    NoSession,
    /// Both factors have not passed.
    NotVerified,
    /// The service holds the most sessions it may.
    Full,
}

#[derive(Default)]
pub struct Sessions {
    map: Mutex<HashMap<Token, Arc<Mutex<Session>>>>,
}

impl Sessions {
    pub async fn open(&self, source: Source, hospital: Arc<Hospital>) -> Result<Token, Refusal> {
        let mut map = self.map.lock().await;
        // Wipe sessions the agent never closed. A held lock means the session is in use.
        map.retain(|_, s| s.try_lock().map_or(true, |s| s.opened.elapsed() < MAX_AGE));
        if map.len() >= MAX_SESSIONS {
            return Err(Refusal::Full);
        }
        let handle = Token::random();
        let session = Session {
            source,
            stage: Stage::Unverified(None),
            identifies: 0,
            resends: 0,
            code_failures: 0,
            offered: HashMap::new(),
            rest: Vec::new(),
            next_pages: Vec::new(),
            facts: None,
            note: None,
            text_template: None,
            hospital,
            booked: HashMap::new(),
            ledger: Vec::new(),
            seen: None,
            mode: Default::default(),
            pcp: Vec::new(),
            search_pcp: None,
            sooner: None,
            search_place: None,
            place_words: None,
            opened: Instant::now(),
        };
        map.insert(handle.clone(), Arc::new(Mutex::new(session)));
        Ok(handle)
    }

    /// Ends the session. Closing twice is not an error.
    /// Closes the session and gives it back, for its ledger.
    pub async fn close(&self, handle: &Token) -> Option<Arc<Mutex<Session>>> {
        self.map.lock().await.remove(handle)
    }

    pub async fn get(&self, handle: &Token) -> Result<Arc<Mutex<Session>>, Refusal> {
        self.map.lock().await.get(handle).cloned().ok_or(Refusal::NoSession)
    }
}

impl Session {
    /// Milliseconds from the session's start to `at`.
    pub fn ms_since_open(&self, at: Instant) -> u32 {
        u32::try_from(at.saturating_duration_since(self.opened).as_millis()).unwrap_or(u32::MAX)
    }

    pub fn patient(&self) -> Result<PatientKey, Refusal> {
        match &self.stage {
            Stage::Verified(p) => Ok(p.clone()),
            Stage::Unverified(_) | Stage::Locked => Err(Refusal::NotVerified),
        }
    }
}
