# Runbook: prove-gate

Trigger: a gate is defined (state Defined) and must become Installed.
Steps:
1. Create a planted defect under `fixtures/planted/<gate>/` that the gate must catch, with a README naming the criterion.
2. Run the gate against the clean fixture: expect pass.
3. Run the gate against the planted fixture: expect fail, with the failure visible where a human would look (PR check, CLI exit code and message).
4. Store the GateProof with both run references.
5. Only then cite the gate in CI_CD, in any document, or in an agent instruction file. The citation gate enforces this.
Verification: `gates.list` shows Installed with a proof; `gates.prove` re-run reproduces both results.
Rollback: revoke the proof (state back to Defined) if the gate is changed; re-prove.
May run: coder agent under a ticket; verification lead accepts the proof.
