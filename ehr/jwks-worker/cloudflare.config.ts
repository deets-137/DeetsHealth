// The JWK Set URL that Epic reads to check the ehr service's sign-in (EPIC.md §2, §5).
// It serves PUBLIC keys only. The private key never comes here (SECURITY.md §4.6).
// Whoever can deploy this Worker can add a key that Epic trusts, so deploy access is
// guarded like the private key itself.
import { defineConfig } from "cf/config";
import * as entrypoint from "./src/index.ts" with { type: "cf-worker" };

export default defineConfig({
	worker: {
		name: "deetshealth-keys",
		compatibilityDate: "2026-09-30",
		entrypoint,
		domains: ["health-keys.deets.solutions"],
	},
});
