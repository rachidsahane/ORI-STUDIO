# OBSERVABILITY: Ori Studio

What is measured, the objectives, the alert rules, and which agent watches what. Ori Studio is a desktop product with no server, so "production" means users' machines and the project's own staging; telemetry from users is never collected (PROJECT_BRIEF section 6). Observability here is about the engine's own health on the operator's machine and about the project's development pipeline.

## 1. Signals in the engine (local, never sent anywhere)

| Signal | Kind | Used by |
|---|---|---|
| Event throughput and log size | Metric | Dashboard, performance budget |
| RPC latency per method | Metric | Dashboard, performance budget |
| Session states, budget consumption, outcomes | Metric and events | Fleet view, calibration |
| Gate runs: pass, fail, error, missing | Events | Review queue, liveness |
| Unattributed changes | Events | Interrupt notification |
| Retrieval package size and latency | Metric | Calibration (retrieval quality) |
| Integration health per slot | State | Dashboard degraded banner |
| Container runtime availability | State | Downgrade banner |
| Engine errors with methodology reason | Structured log | Local log file, "explain" action |

Local logs are structured (JSON lines), rotated, never contain secrets, and are exportable by the operator on request for a bug report.

## 2. Objectives (for the project's own staging and releases)

| Objective | Target set at phase 1 baseline |
|---|---|
| Dashboard update latency after an event | Under two seconds |
| Idle CPU with no sessions | Negligible |
| Startup to usable dashboard | Under three seconds |
| Crash-free sessions of the desktop app in the screenshot matrix | Baseline, then only up |
| Gate liveness: every expected workflow ran inside its window | 100 percent |

## 3. Alert rules and routing

| Condition | Route |
|---|---|
| Unattributed change detected | Interrupt |
| Gate `missing` (inert) | Interrupt |
| Credential exposure suspected | Interrupt |
| Session blocked or escalated | Review window |
| QA run finished | Review window |
| Drift audit result | Review window |
| Digest | Twice daily |

## 4. Who watches what

| Agent | Watches | Files |
|---|---|---|
| operations | Staging health, release pipeline, liveness | Incident tickets |
| qa | Defects on staging, screenshot matrix, simulation findings | Defect tickets, proposed criteria |
| product_signal | Not applicable in the first release (no user telemetry) | |
| documentation | Drift between `spec/` and code, weekly | Drift audit report |
