# Pull request report

What the coder attaches to the pull request it opens. It is the document the lead agent reviews and the human approves against the risk tier.

Source: AICD appendix A.3, reproduced field for field. Nothing is added.

## Fields

| Field | Required | Content |
|---|---|---|
| Ticket and anchor | Yes | Links |
| Plan | Yes | The plan written before implementation, and deviations from it |
| Tests | Yes | Added and modified tests, with the reason for each |
| Coverage matrix | Yes | Criterion identifier to test name |
| Not tested | Yes (`none` if none) | What was deliberately not tested and why |
| Risk tier | Yes | Self-assessed, with justification |
| Rollback plan | Required for tier 2 | |
| Escalations | Yes (`none` if none) | Any trigger encountered and how it was resolved |

## Form

| Field | Value |
|---|---|
| Ticket and anchor | |
| Plan | |
| Tests | |
| Risk tier | |
| Rollback plan | |
| Escalations | |

### Coverage matrix

| Criterion | Test name |
|---|---|
| | |

### Not tested

| What | Why |
|---|---|
| | |
