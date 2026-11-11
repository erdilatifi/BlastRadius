import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";

const root = document.getElementById("root");

if (root) {
	createRoot(root).render(
		<StrictMode>
			<h1 className="p-8 text-2xl font-semibold">BlastRadius</h1>
		</StrictMode>,
	);
}
