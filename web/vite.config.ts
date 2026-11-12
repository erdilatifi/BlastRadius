import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

// `npm run dev` proxies the API to a control plane on port 8080, the same
// port-forward brctl uses.
export default defineConfig({
	plugins: [tailwindcss()],
	server: { proxy: { "/api": "http://localhost:8080" } },
});
