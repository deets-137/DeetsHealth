"""Shared fixtures: a real ehr on a free port with its fake patients, reached over mTLS.

Needs `cargo build -p deetshealth-ehr` and the dev certificates
(`cargo run -p deetshealth-ehr --example dev_certs`).
"""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from deets_agent import config
from deets_agent.ehr_client import EhrClient
from deets_agent.ehr_process import running_ehr


@pytest.fixture
async def ehr():
    async with running_ehr() as target:
        client = await EhrClient.connect(target, config.CERTS)
        try:
            yield client
        finally:
            await client.close()
