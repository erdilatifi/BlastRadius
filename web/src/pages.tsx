import type { ReactNode } from "react";
import { type Amount, useCapacity, useTenants } from "./api";

export const show = (v: number | null, unit = "") =>
	v === null ? "–" : `${v}${unit}`;

const grafana = `${location.protocol}//grafana.${location.host}`;

export function Status<T>({
	query,
	children,
}: {
	query: { data?: T; error: Error | null };
	children: (data: T) => ReactNode;
}) {
	if (query.error) return <p className="text-red-600">{query.error.message}</p>;
	if (!query.data) return <p className="text-gray-500">Loading…</p>;
	return children(query.data);
}

export function Table({
	head,
	rows,
}: {
	head: string[];
	rows: [string, ReactNode[]][];
}) {
	return (
		<table className="w-full text-left text-sm">
			<thead className="border-b text-gray-500">
				<tr>
					{head.map((h) => (
						<th key={h} className="py-2 pr-4 font-medium">
							{h}
						</th>
					))}
				</tr>
			</thead>
			<tbody>
				{rows.map(([key, cells]) => (
					<tr key={key} className="border-b last:border-0">
						{cells.map((c, i) => (
							<td key={head[i]} className="py-2 pr-4 tabular-nums">
								{c}
							</td>
						))}
					</tr>
				))}
			</tbody>
		</table>
	);
}

const healthColour: Record<string, string> = {
	Healthy: "text-green-700",
	Progressing: "text-amber-600",
	Degraded: "text-red-600",
};

function Budget({ left }: { left: number | null }) {
	if (left === null) return "–";
	const colour =
		left < 25 ? "bg-red-500" : left < 50 ? "bg-amber-500" : "bg-green-600";
	return (
		<div className="flex items-center gap-2">
			<div className="h-2 w-24 rounded bg-gray-200">
				<div
					className={`h-2 rounded ${colour}`}
					style={{ width: `${left}%` }}
				/>
			</div>
			{left}%
		</div>
	);
}

export function Tenants() {
	return (
		<Status query={useTenants()}>
			{(rows) => (
				<Table
					head={[
						"Tenant",
						"Tier",
						"Sync",
						"Health",
						"Availability",
						"Target",
						"p95",
						"Budget left",
						"Burn rate",
					]}
					rows={rows.map((t) => [
						t.name,
						[
							<a
								key="name"
								className="text-blue-700 hover:underline"
								href={`${grafana}/d/tenant-slo?var-tenant=${t.name}`}
							>
								{t.name}
							</a>,
							t.tier,
							t.sync,
							<span
								key="health"
								className={healthColour[t.health] ?? "text-gray-500"}
							>
								{t.health}
							</span>,
							show(t.availability, "%"),
							show(t.target, "%"),
							show(t.p95Ms, " ms"),
							<Budget key="budget" left={t.budgetRemaining} />,
							<span
								key="burn"
								className={
									(t.burnRate ?? 0) > 1 ? "font-semibold text-red-600" : ""
								}
							>
								{show(t.burnRate, "×")}
							</span>,
						],
					])}
				/>
			)}
		</Status>
	);
}

function Bar({
	label,
	used,
	total,
	gate,
}: {
	label: string;
	used: number;
	total: number;
	gate: number;
}) {
	const percent = Math.min(100, (used / total) * 100);
	return (
		<div className="space-y-1">
			<div className="flex justify-between text-sm">
				<span>{label}</span>
				<span className="tabular-nums">
					{used} / {total} ({percent.toFixed(0)}%)
				</span>
			</div>
			<div className="relative h-3 rounded bg-gray-200">
				<div
					className={`h-3 rounded ${percent > gate ? "bg-red-500" : "bg-blue-500"}`}
					style={{ width: `${percent}%` }}
				/>
				<div
					className="absolute top-0 h-3 w-px bg-gray-800"
					style={{ left: `${gate}%` }}
				/>
			</div>
		</div>
	);
}

const fields: [keyof Amount, string][] = [
	["cpuMillis", "CPU (millicores)"],
	["memoryMiB", "Memory (MiB)"],
	["pods", "Pods"],
];

export function Capacity() {
	return (
		<Status query={useCapacity()}>
			{(c) => (
				<div className="space-y-8">
					<section className="space-y-3">
						<h2 className="font-semibold">
							Committed quota (gate at {c.gatePercent}%)
						</h2>
						{fields.map(([key, label]) => (
							<Bar
								key={key}
								label={label}
								used={c.committed[key]}
								total={c.capacity[key]}
								gate={c.gatePercent}
							/>
						))}
						<p className="text-sm text-gray-600">
							Still fits:{" "}
							{Object.entries(c.fits)
								.map(([tier, n]) => `${n} ${tier}`)
								.join(", ")}
						</p>
					</section>
					<Table
						head={[
							"Tenant",
							"Tier",
							"CPU used",
							"CPU requested",
							"Memory used",
							"Memory requested",
						]}
						rows={c.tenants.map((t) => [
							t.name,
							[
								t.name,
								t.tier,
								show(t.cpuUsedMillis, "m"),
								show(t.cpuRequestedMillis, "m"),
								show(t.memoryUsedMib, " Mi"),
								show(t.memoryRequestedMib, " Mi"),
							],
						])}
					/>
				</div>
			)}
		</Status>
	);
}
