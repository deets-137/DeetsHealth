"""FileCall (DATABASE.md §4a): the tests' ehr runs without a store, so filing a call is refused
quietly and the agent goes on. The filing itself is covered by ehr's own store test
(`npm run db:test`) and by a typed call with the store on."""

from deets_agent.ehr_client import EhrError


async def test_without_a_store_a_call_is_not_filed_and_nothing_fails(ehr, tmp_path):
    record = tmp_path / "20261112-000000-typed.jsonl"
    record.write_text('{"t_ms": 0, "kind": "call_start", "schema_version": 3}\n', encoding="utf-8")
    assert await ehr.file_call(record.stem, record) is None


async def test_a_bad_call_id_is_refused_at_the_edge(ehr, tmp_path):
    record = tmp_path / "bad.jsonl"
    record.write_text('{"t_ms": 0}\n', encoding="utf-8")
    try:
        await ehr.file_call("../../etc/passwd", record)
    except EhrError:
        return
    raise AssertionError("a call id with a path in it must be refused")
