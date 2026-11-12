import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import {
	createBrowserRouter,
	Navigate,
	NavLink,
	Outlet,
	RouterProvider,
} from "react-router";
import { useRuns, useTenants } from "./api";
import { Backdrop } from "./backdrop";
import { Chaos } from "./chaos";
import { Incidents } from "./incidents";
import "./index.css";
import { Capacity, Tenants } from "./pages";
import { cn, Dot } from "./ui";

const queries = new QueryClient({
	defaultOptions: { queries: { refetchInterval: 2000 } },
});

const nav = [
	["tenants", "01", "Tenants"],
	["chaos", "02", "Chaos"],
	["incidents", "03", "Incidents"],
	["capacity", "04", "Capacity"],
] as const;

function Mark() {
	return (
		<svg viewBox="0 0 32 32" className="size-7" aria-hidden="true">
			<g fill="none" stroke="#00afae">
				<circle cx="16" cy="16" r="14.5" strokeOpacity="0.35" />
				<circle cx="16" cy="16" r="9.5" strokeDasharray="2 2.5" />
				<path d="M16 0v6M16 26v6M0 16h6M26 16h6" strokeOpacity="0.7" />
			</g>
			<rect
				x="12.5"
				y="12.5"
				width="7"
				height="7"
				fill="#00afae"
				transform="rotate(45 16 16)"
			/>
		</svg>
	);
}

function Clock() {
	const [now, setNow] = useState(() => new Date());
	useEffect(() => {
		const id = setInterval(() => setNow(new Date()), 1000);
		return () => clearInterval(id);
	}, []);
	return <>{now.toISOString().slice(11, 19)}</>;
}

function Readouts() {
	const tenants = useTenants();
	const runs = useRuns().data ?? [];
	const rows = tenants.data ?? [];
	const tracked = rows.filter((t) => t.health !== "n/a");
	const healthy = tracked.filter((t) => t.health === "Healthy").length;
	const live = runs.some((r) => !["done", "failed"].includes(r.phase));
	const online = !tenants.error && tenants.data !== undefined;
	const link = (
		<span
			className={cn(
				"flex items-center gap-2 border px-2 py-1",
				online ? "border-accent-line text-accent" : "border-crit/50 text-crit",
			)}
		>
			<Dot tone={online ? "accent" : "crit"} live />
			{online ? "LINK" : "NO LINK"}
		</span>
	);
	return (
		<>
			{/* Phones and tablets: the two readouts that need attention. */}
			<div className="ml-auto flex items-center gap-3 font-mono text-[10px] whitespace-nowrap lg:hidden">
				{live && (
					<span className="flex items-center gap-1.5 text-crit">
						<Dot tone="crit" live />
						ARMED
					</span>
				)}
				{link}
			</div>
			<div className="ml-auto hidden items-center gap-6 font-mono text-[11px] whitespace-nowrap lg:flex">
				<span className="flex items-baseline gap-2">
					<b className="text-[10px] font-normal tracking-widest text-text-3">
						FLEET
					</b>
					{tracked.length
						? `${healthy}/${tracked.length} OK`
						: `${rows.length} TENANTS`}
				</span>
				<span className="flex items-baseline gap-2">
					<b className="text-[10px] font-normal tracking-widest text-text-3">
						CHAOS
					</b>
					<span className={live ? "text-crit" : "text-text-2"}>
						{live ? "ARMED" : "IDLE"}
					</span>
				</span>
				<span className="flex items-baseline gap-2">
					<b className="text-[10px] font-normal tracking-widest text-text-3">
						UTC
					</b>
					<Clock />
				</span>
				{link}
			</div>
		</>
	);
}

function Layout() {
	return (
		<div className="relative isolate min-h-screen">
			<Backdrop />
			<header className="sticky top-0 z-20 border-b border-line bg-bg/75 backdrop-blur-md">
				<div className="mx-auto flex h-14 max-w-[1400px] items-center gap-5 px-4 sm:px-6">
					<NavLink to="/tenants" className="flex items-center gap-3">
						<Mark />
						<span className="text-[15px] font-semibold tracking-tight [font-stretch:118%]">
							BLAST<span className="text-accent">RADIUS</span>
						</span>
					</NavLink>
					<span className="hidden h-5 w-px bg-line-3 sm:block" />
					<span className="hidden border border-line-2 px-1.5 py-1 font-mono text-[10px] text-text-3 sm:block">
						CONTROL PLANE
					</span>
					<Readouts />
				</div>
				<nav className="mx-auto flex max-w-[1400px] gap-px px-2 sm:px-6">
					{nav.map(([path, code, label]) => (
						<NavLink
							key={path}
							to={`/${path}`}
							className={({ isActive }) =>
								cn(
									"group relative flex min-h-11 flex-1 items-center justify-center gap-2 px-1 transition-colors sm:flex-none sm:px-3",
									isActive ? "text-text" : "text-text-3 hover:text-text-2",
								)
							}
						>
							{({ isActive }) => (
								<>
									<span
										className={cn(
											"hidden font-mono text-[10px] sm:inline",
											isActive ? "text-accent" : "text-text-3",
										)}
									>
										{code}
									</span>
									<span className="label">{label}</span>
									<span
										className={cn(
											"absolute inset-x-2 -bottom-px h-0.5 transition-all",
											isActive
												? "bg-accent shadow-[0_0_12px_var(--color-accent)]"
												: "bg-transparent group-hover:bg-line-3",
										)}
									/>
								</>
							)}
						</NavLink>
					))}
				</nav>
			</header>
			<main className="mx-auto max-w-[1400px] space-y-6 px-4 py-6 sm:px-6 sm:py-8">
				<Outlet />
			</main>
			<footer className="mx-auto flex max-w-[1400px] flex-wrap justify-between gap-2 px-4 pb-8 font-mono text-[10px] text-text-3 sm:px-6">
				<span>GITOPS · ARGOCD · KYVERNO · CHAOS MESH · PROMETHEUS</span>
				<span>BLAST RADIUS TARGET = 0</span>
			</footer>
		</div>
	);
}

const router = createBrowserRouter([
	{
		element: <Layout />,
		children: [
			{ index: true, element: <Navigate to="/tenants" replace /> },
			{ path: "tenants", element: <Tenants /> },
			{ path: "chaos", element: <Chaos /> },
			{ path: "incidents", element: <Incidents /> },
			{ path: "capacity", element: <Capacity /> },
		],
	},
]);

const root = document.getElementById("root");

if (root) {
	createRoot(root).render(
		<StrictMode>
			<QueryClientProvider client={queries}>
				<RouterProvider router={router} />
			</QueryClientProvider>
		</StrictMode>,
	);
}
