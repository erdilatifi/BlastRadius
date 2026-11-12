import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import {
	createBrowserRouter,
	Navigate,
	NavLink,
	Outlet,
	RouterProvider,
} from "react-router";
import { Chaos } from "./chaos";
import "./index.css";
import { Capacity, Tenants } from "./pages";

const queries = new QueryClient({
	defaultOptions: { queries: { refetchInterval: 2000 } },
});

const link = ({ isActive }: { isActive: boolean }) =>
	isActive
		? "font-semibold text-blue-700"
		: "text-gray-600 hover:text-gray-900";

function Layout() {
	return (
		<div className="mx-auto max-w-6xl space-y-6 p-6">
			<header className="flex items-baseline gap-6 border-b pb-3">
				<h1 className="text-xl font-semibold">BlastRadius</h1>
				<NavLink to="/tenants" className={link}>
					Tenants
				</NavLink>
				<NavLink to="/chaos" className={link}>
					Chaos
				</NavLink>
				<NavLink to="/capacity" className={link}>
					Capacity
				</NavLink>
			</header>
			<Outlet />
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
