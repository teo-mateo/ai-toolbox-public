---
name: software-planning
description: Create, revise, review, or compare evidence-backed implementation plans for software features, fixes, refactors, migrations, and integrations. Use when asked for a technical plan, design review, implementation roadmap, or plan-readiness assessment before coding. Works in any project by tracing real code paths, defining explicit contracts, and linking requirements to changes and tests.
---

# Software planning

Produce a plan another engineer can implement without inventing missing contracts.
Optimize for correctness, completeness of the requested behavior, and the smallest
coherent change, not document length or the number of proposed features.

## Invocation and boundaries

Use `/skill:software-planning <task, plan path, or plans to compare>`.
Infer the mode from the request: **create**, **revise**, **review**, or **compare**.
If the task itself is missing, ask for it rather than inventing a project change.

- Follow the current project's instructions, conventions, and requested scope.
- Plan, do not implement, unless implementation is separately requested. Write
  the requested planning artifact, not production code or runtime configuration.
- Inspect the repository and working-tree state before proposing changes. Do not
  reset, stash, switch branches, commit, deploy, or restart services just to plan;
  do so only when separately authorized or required by applicable instructions.
- Prefer read-only inspection and isolated tests. Identify commands that mutate
  data, consume paid resources, or disturb running work before proposing a probe.
  Run those only with appropriate authorization.
- Scale the depth to the risk. A small fix may need a short plan; cross-layer work
  needs explicit contract and path tables. Consider the checks below, but omit
  irrelevant sections rather than manufacturing architecture or busywork.
- Use the user's output path or the repository's planning convention. If a file
  is requested and neither exists, use `docs/plans/<topic>.md`. That is a path in
  the **target project**, not in this skill's directory. Otherwise respond in chat.

## 1. Establish the task and evidence baseline

Read the request, applicable repository guidance, related plans, and the relevant
build/test configuration. Record:

- The user-visible outcome and observable acceptance criteria.
- Constraints and explicitly excluded surfaces, clients, and side effects.
  Include relevant security, reliability, performance, resource, and compatibility
  requirements without inventing new ones.
- Repository revision, branch, and relevant uncommitted or untracked files. For
  comparisons, read each exact plan version and identify whether their code bases
  differ. Inspect other refs without disturbing the working tree.
- Decisions already made by the user versus your recommendations. Never label an
  assumption as an agreed decision.

Use brief requirement IDs (`R1`, `R2`, ...) when needed for traceability.
Ask only questions that materially change architecture, compatibility, safety,
behavior, or scope. Carry non-blocking choices as clearly labeled proposals.
Surface conflicts with agreed constraints rather than silently relaxing them.

For important claims, distinguish:

| Evidence | What it establishes |
|---|---|
| Source inspected | Behavior implemented at the named revision and symbol |
| Test/probe run | Behavior observed with the named command, version, and conditions |
| Documentation | A stated contract for the documented version |
| Assumption/unknown | Not yet established; requires a decision or verification |

Cite `path: symbol` and optionally line ranges for decisive source claims. Mark
new files/functions as proposed. A map being built does not prove it is returned;
a field being accepted does not prove it is stored; a success response does not
prove an option is honored. Do not copy earlier plans' claims without checking.
Do not generalize one target's behavior to every adapter, backend, or version.
Upstream source is not proof of what an older deployed binary does.

Stop discovery when each material path and decision is supported or identified
as a blocker. Do not inventory unrelated subsystems.

## 2. Trace the behavior end to end

Start at the real entry point and follow the data to the observable effect and
back to the caller. Search callers, constructors, serializers, writers, and tests;
read the relevant function bodies rather than relying on names or comments.

A typical trace is:

`input -> validation/normalization -> storage -> runtime context -> execution -> output`

Adapt it to the project: a library function, CLI, worker, native app, or API does
not need an invented web frontend. Include every materially distinct applicable
path, especially:

- Initial creation/first use versus updating an existing object.
- Normal execution versus edit-and-rerun, retry, fallback, batch, or loop paths.
- Copies, child objects, imports, restores, and transient/in-memory modes.
- Direct API/CLI callers as well as UI clients; older clients still in use.
- Local and remote targets, supported and unsupported adapters.
- Background execution, reload/reattach, cancellation, and auxiliary operations.
- Snapshot/list/detail projections and incremental/cache update paths.

Use a compact path inventory for nontrivial changes:

| Path | Entry point | State/validation owner | Final consumer | Change or exclusion | Verification |
|---|---|---|---|---|---|

Do not assume a component reuses another component or calls a similarly named
endpoint. Verify imports and actual calls, including HTTP methods and payloads.
For exclusions, explain how the old behavior remains intact.

## 3. Specify contracts before listing edits

### Representation, defaults, and persistence

For each new or changed value, decide its type, allowed domain, scope/lifetime,
canonical representation, and precedence. Define applicable cases explicitly:

- Missing versus `null`, empty, whitespace-only, zero, and `false`.
- Case normalization, aliases, duplicate removal, and ordering.
- Invalid input versus valid-but-unsupported input.
- Omitted update fields versus explicit clearing; merge versus replacement.
- Stored value versus effective value after defaults or capability resolution.

Prefer one representation for a default. If multiple representations are needed,
define their conversion. Do not conflate a default with disabled/off. If a global
default is requested, explain how it is filtered for each target that inherits it.

Trace every persistence step: fresh schema, existing-data migration, insert/create,
update allowlist, row/object loading, serialization, and public projections. Adding
a column and dataclass field alone is not a complete storage change. Identify
atomic writes, transactions/locks, concurrent writers, and migration compatibility
where relevant. Separate human-edited config from generated and machine-local data.

### Capabilities and execution boundaries

Name the source of truth for target capabilities and how the execution layer gets
its identity/version. A generic adapter needs those inputs; naming a helper is not
plumbing them. Remote targets may have no entry in the local configuration store.

- Define mappings only for supported targets. Verify interactions between paired
  flags, explicit enable/disable, inherited defaults, and wire-level types.
- Decide unsupported/unknown behavior explicitly: reject, defer, or use a visible
  documented fallback. Never assume downstream ignores or repairs arbitrary input.
- Enforce opt-in/disabled behavior at execution, not merely by hiding a UI control.
- Validate untrusted values on write and resolve stale capability-dependent state
  before execution when necessary. Use one authoritative policy across entry paths;
  UI validation is a convenience, not the enforcement boundary.
- Preserve existing authorization, tenant isolation, and input limits. Validate
  before starting side effects, persisting work, or truncating existing results.
- Keep parsing/mapping pure where useful; disk, network, and settings-store I/O
  belong to explicit owners, not a module advertised as having no side effects.

If a feature is declared unchanged for a target, prove that a crafted request or
old persisted value cannot accidentally activate it there.

### State transitions, concurrency, and visibility

Specify what happens when the selected object/target changes, its capabilities
change, a config entry is removed, or a previously valid value becomes unsupported.
Keep dependent values consistent in one atomic update where possible. The UI must
not show one effective choice while execution uses another hidden stored value.

For asynchronous work, name when inputs are captured and whether they remain
fixed for the whole operation, including retries, loops, and final fallbacks.
Decide whether mid-operation edits affect the current or next operation; match
control availability and wording to that rule.

For optimistic updates, account for failure, rapid successive edits, navigation,
late fetches, and out-of-order responses. Rollback must not overwrite a newer
choice or another object's state. Cover immediate submit after a setting change:
either await persistence or carry a coherent input snapshot into execution.

Trace how a write becomes visible: response, refetch, event, cache invalidation,
or a combination. A field added to the initial snapshot is not automatically
present in incremental updates. Metadata-only changes should avoid unnecessary
builds/restarts; inspect the actual save path, not just its success message.

## 4. Choose and sequence the smallest coherent design

Compare alternatives only where they affect a real decision. Explain why the
chosen design fits the verified code and requested outcome. Defer optional global
settings, history, automation, probes, generalized frameworks, and extra clients
unless needed for correctness or explicitly requested.

For each implementation step, provide:

| Step / requirements | Files and symbols | Behavior and responsibility | Dependencies | Acceptance test |
|---|---|---|---|---|

A phase must not depend on a later phase's helper, schema, settings, or capability
source. Mark preparatory phases as such; do not call them independently usable
when their entry path is missing. Prefer reviewable vertical slices and reuse the
project's existing boundaries rather than inventing parallel update paths.

Name rollout ordering, compatibility with older clients/data/workers, enablement,
and rollback/data-recovery limits when applicable. A rollback must not imply an
unsafe destructive migration. List operational prerequisites without performing
deployment as part of planning.

## 5. Link requirements to falsifiable tests

Every requirement and important risk needs a test or explicit verification task.
Specify the setup, action, and observable expected result, plus the test location
or command. Separate **tests proposed** from **checks actually run**; never report
planned validation as passing.

Select applicable coverage:

- Pure parsing/mapping: types, boundary values, normalization, aliases, invalid
  values, defaults, disabled behavior, and target-specific support.
- Persistence: create-save-reload round trip, update-clear-preserve semantics,
  fresh and old schemas, idempotent migrations, and every public projection.
- Boundaries: actual outgoing payload or invoked dependency; assert both required
  fields and forbidden/omitted fields. Explicitly test unchanged legacy behavior.
- Integration: first use, subsequent use, edit/rerun, copy/inheritance, alternate
  entry points, and every loop/retry/finalization path carrying the same inputs.
- State/UI: no-choice states, return to default, target/capability changes,
  pending work, rapid updates, navigation races, failure rollback, and live refresh.
- Security/operations: authorization, malformed direct calls, transactional
  failure, lost updates, metadata-only side effects, and deployment prerequisites.
- A bounded real smoke test when mocks cannot establish an external contract.

A direct dependency probe does not prove the application forwards the option.
A helper test does not prove callers use it. A status code or rendered control
alone does not prove the requested behavior. Use deterministic assertions where
possible; do not promise monotonic or exact outcomes from nondeterministic systems
without evidence and an appropriate test method.

## 6. Run an adversarial readiness pass

Walk the written plan as if implementing it literally. Check:

1. Can every new input travel from all included entry points through storage and
   execution to its observable effect? Is a create/insert/loader step missing?
2. Does the first action work before any persisted object exists?
3. Can switching targets or removing a capability leave an invisible active value?
4. Can a direct client, edit, retry, copy, or import bypass the intended policy?
5. Does omitted/default input preserve the previous contract at the real boundary?
6. Does a config edit reach already-connected consumers without an accidental
   rebuild, restart, or lost concurrent update?
7. Are operation snapshots and optimistic rollbacks safe under timing changes?
8. Do prose, examples, tables, phases, and tests agree on names, types, defaults,
   API methods, aliases, accepted values, and failure behavior?
9. Are decisive claims backed by the right revision/version, and are unverified
   behaviors labeled as unknown rather than convenient downstream fallbacks?
10. Is any excluded concern actually required to deliver an included requirement?

Fix contradictions before delivery. If independent review is worthwhile and
available, give a read-only reviewer the exact plan and code baseline; verify its
findings against source yourself. Do not require subagents or a particular model.

Use one readiness verdict:

- **Ready for implementation:** contracts and paths are coherent; remaining checks
  are explicit implementation/acceptance gates, not unresolved design decisions.
- **Needs revision:** known gaps or contradictions prevent a literal implementation.
- **Blocked on decision/evidence:** a named unresolved question materially changes
  the design. State the smallest decision or safe probe that would unblock it.

## Plan output structure

Use these headings as a scaffold, collapsing sections for small tasks:

1. **Goal and scope:** requirements, acceptance criteria, constraints, non-goals.
2. **Verified current behavior:** baseline, decisive evidence, execution/data paths.
3. **Proposed design and contracts:** representation, capabilities, state transitions,
   compatibility, and rationale; distinguish recommendations from agreed decisions.
4. **Implementation sequence:** concrete owners/files/symbols and phase dependencies.
5. **Verification:** requirement-to-test mapping and checks run versus proposed.
6. **Rollout and risks:** enablement, rollback, unknowns, exclusions, and mitigations.
7. **Readiness:** verdict and any exact revisions or decisions still needed.

An artifact is not implementation-ready merely because every heading is filled.

## Reviewing or comparing existing plans

Read every plan completely and check decisive claims against its code baseline.
Evaluate each **as written**, not the improved design you could construct from it.
Separate fatal contract gaps, fixable omissions, factual errors, and optional polish.
Do not reward length, plausible specificity, author/model identity, or extra scope.

When asked to rate plans, use the same rubric for all candidates:

| Dimension | Weight |
|---|---:|
| Contract correctness and internal consistency | 25% |
| End-to-end coverage of requested behavior | 20% |
| Source fidelity and fit with existing architecture | 20% |
| Safety, compatibility, and state/concurrency handling | 15% |
| Verification quality and acceptance criteria | 10% |
| Scope discipline and implementation clarity | 10% |

Rate each dimension from 0 to 10; compute the weighted overall score and
round to the nearest half-point. Anchor the scale: 0 = absent/contradictory,
5 = partly specified with material gaps, 8 = implementable with minor omissions,
10 = unusually complete and verified for the requested scope. Do not let a good
average override a blocking flaw; report readiness separately.

Use a shared scope for comparison. An explicit deferral is not itself a flaw when
it meets the request; excluding required behavior is. Keep the rubric consistent
when a new candidate arrives, and explain any revised earlier score.

Deliver a ranking, the few decisive findings with file/symbol evidence, and one
clear recommendation. Name useful ideas to borrow and required fixes. Distinguish
**best as written** from **best after specified revisions** without presenting an
unwritten hybrid as the current winner.
