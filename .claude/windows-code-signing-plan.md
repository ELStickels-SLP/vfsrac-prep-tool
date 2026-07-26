# Windows code signing plan (Azure Trusted Signing, individual)

Status: not started. Notes for when we're ready to remove the SmartScreen
"unrecognized publisher" warning on `voicePitchFeedback-windows.exe`.

## Why Azure Trusted Signing

- Built for indie/solo devs — no D-U-N-S number or business registration
  needed, unlike traditional OV/EV certs.
- ~$10/month subscription (Azure "Personal" identity validation tier).
- No hardware token / cloud HSM to manage — Microsoft holds the key,
  signing happens via an Azure API call in CI.
- SmartScreen reputation builds faster since Microsoft is both the CA and
  the OS vendor doing the reputation check.

## Setup steps

1. **Azure account** — need an Azure subscription (pay-as-you-go is fine
   for the ~$10/mo cost).
2. **Create a Trusted Signing account** in the Azure portal (resource type
   "Trusted Signing Account"). Pick a region that supports it (currently a
   limited set, e.g. East US, West US 2 — check current availability).
3. **Identity validation (individual/"Public Trust" tier)**:
   - Requires a government-issued photo ID.
   - Requires a live video verification call (via Microsoft's identity
     verification partner) — schedule this, it's not instant.
   - Typically takes a few business days to clear after the video call.
4. **Create a certificate profile** under the Trusted Signing account,
   type "Public Trust" (for individuals — "Private Trust"/"VBS" profiles
   are for internal-only, won't clear SmartScreen for public distribution).
5. **Create a Microsoft Entra ID app registration** (service principal) with
   permission to invoke the Trusted Signing account, and generate a client
   secret (or use OIDC federated credentials with GitHub Actions — OIDC is
   preferred, avoids storing a long-lived secret).
6. **GitHub repo secrets** to add (if not using OIDC federation):
   - `AZURE_TENANT_ID`
   - `AZURE_CLIENT_ID`
   - `AZURE_CLIENT_SECRET`
   - Trusted Signing account/profile names can be plain workflow inputs
     (not secret).
7. **CI integration** — add a signing step to the `windows-latest` job in
   [.github/workflows/release.yml](../.github/workflows/release.yml), after
   "Stage binary (windows)" and before "Upload artifact":
   - Use the official `azure/trusted-signing-action` GitHub Action.
   - Auth via `azure/login@v2` (OIDC, `permissions: id-token: write` on the
     job) then invoke the signing action against
     `dist/voicePitchFeedback-windows.exe`.
   - Rough shape:
     ```yaml
     - uses: azure/login@v2
       with:
         client-id: ${{ secrets.AZURE_CLIENT_ID }}
         tenant-id: ${{ secrets.AZURE_TENANT_ID }}
         subscription-id: ${{ secrets.AZURE_SUBSCRIPTION_ID }}
     - uses: azure/trusted-signing-action@v0
       with:
         endpoint: https://<region>.codesigning.azure.net/
         trusted-signing-account-name: <account-name>
         certificate-profile-name: <profile-name>
         files-folder: dist
         files-folder-filter: exe
     ```
     (Confirm exact action inputs against current docs when implementing —
     the action's interface has changed across versions.)
8. **Verify**: download the signed exe on a clean Windows machine (or VM)
   and confirm `signtool verify /pa` passes and SmartScreen no longer shows
   the blocking prompt (may still show a soft "Windows protected your PC"
   with publisher name shown, rather than "Unknown publisher", until
   enough download reputation accrues — this is expected and resolves
   over time/volume).

## Open questions to resolve before starting

- Which Azure region to host the Trusted Signing account in.
- Whether to use OIDC federated credentials (recommended) vs. a stored
  client secret for the GitHub Actions auth.
- Confirm current pricing/tier names, since Azure Trusted Signing is a
  newer product and naming/pricing has shifted since GA.
