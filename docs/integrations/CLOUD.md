---
status: project
desk_test: none
sources: []
updated: 2026-11-12
---
# The cloud — where each hospital's deploy runs

> **Chosen 2026-10-19: AWS `us-east-1`** (fork C1 A, the owner: "C1A is fine"; §4). The deploy
> is A2: a separate account per hospital in our cloud, behind an interface so a hospital's own
> cloud account (B) can follow (VOICE-AGENT.md §7.2). Comparisons below are from secondary
> sources whose numbers disagree; check each provider's own HIPAA-eligible list.

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

## 4. The fork
Decided 2026-10-19: **A**.
- **C1. Our cloud for A2:** A **AWS `us-east-1`** (recommended: the broadest HIPAA list, next to
  Twilio's media edge, Cartesia on one bill, Claude Platform on AWS if we want Claude inside the
  same cloud) · B **Azure East US** (the cloud many hospitals already run; staff single sign-on
  through Entra ID is native) · C **Google Cloud** (strong data tools; the least common in
  hospitals).
  - Every option keeps B possible: the deploy interface hides the cloud.
  - Whichever we pick, the Anthropic API stays the direct API under its own BAA unless the owner
    chooses a cloud's Claude (ANTHROPIC.md §2).

## 5. Running the parts: Kubernetes, ECS with Fargate, or plain virtual machines
> **Part:** idea · 2026-11-12 — a choice for the deploy work (BUILD-ORDER.md §3.4), not now.
> The owner asked what Kubernetes is and how it relates; these are the notes. It comes back as
> a fork, with costs, when the deploy work starts.

**The parts to run in each hospital's deploy:** the Pipecat agent workers (Python), the Rust
`ehr` service, Postgres (managed), the console, and later the queue for the watch stream
(CHAT.md §5). Something must start them, connect them, scale them and update them.

**The constraint that shapes every option: a call is a long connection.** A phone call holds a
WebSocket from Twilio for minutes. Whatever removes a worker (scaling down, an update) must
first stop new calls going to it, then wait for its calls to end. Done wrong, a patient is cut
off mid-call. The idea is called *draining*.

**Kubernetes** (EKS on AWS, AKS on Azure, GKE on Google). A system that runs containers across
a cluster of servers: we declare what should run (4 agent workers, 2 `ehr`), and it starts
them, restarts a crashed one, adds copies under load and replaces old versions a few at a time.
- For: **deploy B.** The same deploy files run on every large cloud, so moving a hospital to
  its own cloud account could be mostly "point them at the hospital's cluster". The strongest
  reason for it. Also scaling by load and updates with no downtime.
- Against: much to learn and to secure (pods, services, ingress, network policies, Helm, the
  cluster's own access control and secrets), all configured correctly under the BAA. On AWS
  each EKS cluster costs about $73 a month before any servers; A2 (an account per hospital)
  means a cluster per hospital, each one to patch. One shared cluster costs less but isolates
  less, and a security review weighs that. Draining needs a deliberate setup (a long
  termination grace period and a hook that stops new calls first).
- Not needed for mTLS: we do mTLS between the agent and `ehr` ourselves, so a service mesh adds
  nothing there.

**ECS with Fargate** (AWS). AWS runs the containers; there is no cluster to manage. HIPAA
eligible; less to learn; scales by rules (CPU, a schedule) with draining built in (a task
stops taking new work and gets a grace period). Against: AWS only, so deploy B needs a second
set of deploy files behind the deploy interface.

**Plain virtual machines (EC2).** The simplest to understand and the most manual.
- **Do they scale up and down on their own, like Cloudflare Workers? No, not by default.** A
  plain VM runs, and is billed by the second, until someone stops it. Scaling is added with an
  **Auto Scaling group**: it adds or removes whole VMs by a rule (CPU, a custom count such as
  live calls, or a schedule).
- **It is slow compared with Workers.** A Worker starts in milliseconds, per request, and costs
  nothing at rest. A new VM takes about one to three minutes to boot and get ready, so the
  group must add VMs before they are needed, never after a call arrives. A call cannot wait
  minutes, so a call worker never scales to zero: some VMs always run.
- **Scaling in must drain** (a lifecycle hook that holds a VM until its calls end).
- **Why not Workers themselves:** they are short-lived and limited in CPU, and they cannot run
  our Python Pipecat process, the Rust `ehr` service with its mTLS, or Postgres. They stay
  right for small edge jobs such as the key-set Worker (EPIC.md §2.3).

**What we know about load (not measured yet).** A call worker mostly waits on the network
(Twilio, Cartesia, Claude), so one VM or container should hold many calls at once; the number
per vCPU is to be measured with the voice work (BUILD-ORDER.md §3.3). A hospital's call volume
follows its office hours, so a schedule (more workers from 8 am, fewer at night) covers most of
it before any load rule.

**The lean, for when the fork comes:** decide by how soon deploy B is real. A hospital that will
soon want DeetsHealth in its own cloud account: Kubernetes earns its cost. A2 for the first
hospitals: ECS with Fargate is simpler and safer to start, and the deploy interface (VOICE-AGENT.md
§7.2) lets us move to Kubernetes later.

## 6. Sources
- [Saga IT — AWS vs Azure vs GCP for healthcare (2026)](https://saga-it.com/blog/healthcare-cloud-migration-guide)
- [Taction — HIPAA cloud architecture: AWS, Azure, GCP](https://www.tactionsoft.com/blog/hipaa-compliant-cloud-architecture-aws-azure-gcp/)
- [Nirmitee — Healthcare cloud architecture 2026](https://nirmitee.io/blog/healthcare-cloud-architecture-aws-azure-gcp-comparison-2026/)
- [Of Ash and Fire — Which cloud for HIPAA](https://www.ofashandfire.com/blog/hipaa-compliant-cloud-architecture-aws-azure-gcp)
- [Twilio — Core latency in AI voice agents (edge placement)](https://www.twilio.com/en-us/blog/developers/best-practices/guide-core-latency-ai-voice-agents)
- [Cartesia on AWS Marketplace](https://aws.amazon.com/marketplace/pp/prodview-bcf67mlwzvkce)
