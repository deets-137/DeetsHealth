// Serves the public key sets Epic reads (EPIC.md §2, row 3). Nothing else.
// One set per Epic client ID: the sandbox's now; production's joins with the deploy.
import nonprod from "./nonprod.jwks.json" with { type: "json" };

const SETS: Record<string, unknown> = {
	"/nonprod/jwks.json": nonprod,
};

export default {
	fetch(request: Request): Response {
		if (request.method !== "GET" && request.method !== "HEAD") {
			return new Response(null, { status: 405, headers: { allow: "GET, HEAD" } });
		}
		const set = SETS[new URL(request.url).pathname];
		if (!set) return new Response(null, { status: 404 });
		return new Response(JSON.stringify(set), {
			headers: {
				"content-type": "application/json",
				// Short, so a retired key leaves Epic's view within minutes.
				"cache-control": "public, max-age=300",
				"x-content-type-options": "nosniff",
			},
		});
	},
} satisfies ExportedHandler;
