---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# The cloud — where each hospital's deploy runs

> Not chosen. The deploy is A2: a separate account per hospital in our cloud, behind an interface
> so a hospital's own cloud account (B) can follow (VOICE-AGENT.md §7.2). This doc holds the open
> fork for our own cloud (§4). Comparisons below are from secondary sources whose numbers
> disagree; check each provider's own HIPAA-eligible list.

## 1. What a deploy needs
- Managed **Postgres** (VOICE-AGENT.md §8.7), a **KMS** with customer-held keys (BYOK), a
  **secret manager**, **object storage** for recordings when a rule keeps them, containers for
  the Rust service, the Pipecat service and the console, and a log and trace store that we
  keep free of protected data.
- One account (or subscription, or project) per hospital, with no network path between them.

## 2. BAAs
All three large clouds sign a BAA and cover their managed Postgres, KMS, secret manager, object
storage and containers. How the BAA is signed differs:
- **AWS:** accept the BAA in AWS Artifact before storing PHI.
- **Azure:** the BAA applies through Microsoft's online terms.
- **Google Cloud:** request it through the console.
Only services on the provider's HIPAA-eligible list may hold PHI.

## 3. What each one brings for us
| | AWS | Azure | Google Cloud |
|---|---|---|---|
| Near Twilio's US1 media edge (Virginia) | `us-east-1` | East US | `us-east4` |
| Claude | Claude Platform on AWS (Anthropic-operated) or Bedrock | Microsoft Foundry | Vertex AI |
| Cartesia billing | AWS Marketplace listing | — | — |
| Hospitals' own clouds (for B later) | common | **common where Microsoft or Epic already runs** | less common |
| HIPAA-eligible services | the broadest list | broad | broad |

## 4. The fork (for the owner, when the deploy work starts)
- **C1. Our cloud for A2:** A **AWS `us-east-1`** (recommended: the broadest HIPAA list, next to
  Twilio's media edge, Cartesia on one bill, Claude Platform on AWS if we want Claude inside the
  same cloud) · B **Azure East US** (the cloud many hospitals already run; staff single sign-on
  through Entra ID is native) · C **Google Cloud** (strong data tools; the least common in
  hospitals).
  - Every option keeps B possible: the deploy interface hides the cloud.
  - Whichever we pick, the Anthropic API stays the direct API under its own BAA unless the owner
    chooses a cloud's Claude (ANTHROPIC.md §2).

## 5. Sources
- [Saga IT — AWS vs Azure vs GCP for healthcare (2026)](https://saga-it.com/blog/healthcare-cloud-migration-guide)
- [Taction — HIPAA cloud architecture: AWS, Azure, GCP](https://www.tactionsoft.com/blog/hipaa-compliant-cloud-architecture-aws-azure-gcp/)
- [Nirmitee — Healthcare cloud architecture 2026](https://nirmitee.io/blog/healthcare-cloud-architecture-aws-azure-gcp-comparison-2026/)
- [Of Ash and Fire — Which cloud for HIPAA](https://www.ofashandfire.com/blog/hipaa-compliant-cloud-architecture-aws-azure-gcp)
- [Twilio — Core latency in AI voice agents (edge placement)](https://www.twilio.com/en-us/blog/developers/best-practices/guide-core-latency-ai-voice-agents)
- [Cartesia on AWS Marketplace](https://aws.amazon.com/marketplace/pp/prodview-bcf67mlwzvkce)
