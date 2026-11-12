import {
	type Amount,
	type CapacityView,
	type TenantRow,
	useCapacity,
	useTenants,
} from "./api";
import { Field } from "./field";
import { Stack3D, type Tower, tenantColors } from "./stack3d";
import {
	Chip,
	Dot,
	fmt,
	Meter,
	PageHead,
	Panel,
	Stat,
	Status,
	Table,
	type Tone,
	textTone,
} from "./ui";

const grafana = `${location.protocol}//grafana.${location.host}`;

const healthTone = (h: string): Tone =>
	h === "Healthy"
		? "accent"
		: h === "Progressing"
			? "warn"
			: h === "Degraded"
				? "crit"
				: "muted";

const burnTone = (b: number | null): Tone =>
	b === null ? "muted" : b >= 14.4 ? "crit" : b > 1 ? "warn" : "accent";

const budgetTone = (left: number | null): Tone =>
	left === null ? "muted" : left < 25 ? "crit" : left < 50 ? "warn" : "accent";

const tenantTone = (t: TenantRow): Tone => {
	const h = healthTone(t.health);
	const b = burnTone(t.burnRate);
	return h === "crit" || b === "crit"
		? "crit"
		: h === "warn" || b === "warn"
			? "warn"
			: h;
};

function Budget({ left }: { left: number | null }) {
	if (left === null) return "–";
	return (
		<div className="flex items-center gap-3">
			<div className="w-28">
				<Meter percent={left} tone={budgetTone(left)} />
			</div>
			<span className={textTone(budgetTone(left))}>{fmt(left, 1, "%")}</span>
		</div>
	);
}

export function Tenants() {
	return (
		<>
			<PageHead
				code="SYS/01"
				title="Tenant fleet"
				sub="Every tenant is a namespace with its own quota, network policy and SLO. Measured at the ingress, refreshed every two seconds."
			/>
			<Status query={useTenants()}>
				{(rows) => {
					const tracked = rows.filter((t) => t.health !== "n/a");
					const healthy = tracked.filter((t) => t.health === "Healthy").length;
					const avail = rows
						.map((t) => t.availability)
						.filter((v) => v !== null);
					const mean = avail.length
						? avail.reduce((a, b) => a + b, 0) / avail.length
						: null;
					const worst = Math.max(0, ...rows.map((t) => t.burnRate ?? 0));
					const p95 = Math.max(0, ...rows.map((t) => t.p95Ms ?? 0));
					return (
						<>
							<div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
								<Stat
									label="Healthy tenants"
									value={tracked.length ? `${healthy}/${tracked.length}` : "–"}
									tone={
										!tracked.length
											? "muted"
											: healthy === tracked.length
												? "accent"
												: "warn"
									}
									hint={
										tracked.length ? "ArgoCD health" : "ArgoCD not configured"
									}
								/>
								<Stat
									label="Mean availability"
									value={fmt(mean, 2)}
									unit="%"
									hint="30-day SLI at ingress"
								/>
								<Stat
									label="Worst p95"
									value={fmt(p95, 1)}
									unit="ms"
									tone={p95 > 100 ? "warn" : "plain"}
									hint="Across all tenants"
								/>
								<Stat
									label="Peak burn rate"
									value={fmt(worst, 1)}
									unit="×"
									tone={burnTone(worst)}
									hint="Page above 14.4×"
								/>
							</div>
							<div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_440px]">
								<Panel title="Tenants" code="T-LIST" flush>
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
											"Burn",
										]}
										align={["l", "l", "l", "l", "r", "r", "r", "l", "r"]}
										rows={rows.map((t) => [
											t.name,
											[
												<a
													key="name"
													className="flex items-center gap-2 font-sans text-[13px] font-medium text-text hover:text-accent"
													href={`${grafana}/d/tenant-slo?var-tenant=${t.name}`}
												>
													<Dot
														tone={tenantTone(t)}
														live={tenantTone(t) !== "accent"}
													/>
													{t.name}
												</a>,
												<Chip key="tier">{t.tier}</Chip>,
												<span
													key="sync"
													className={
														t.sync === "Synced" ? "text-text-2" : "text-warn"
													}
												>
													{t.sync}
												</span>,
												<Chip key="health" tone={healthTone(t.health)} dot>
													{t.health}
												</Chip>,
												<span key="a" className="text-text">
													{fmt(t.availability, 2, "%")}
												</span>,
												fmt(t.target, 1, "%"),
												fmt(t.p95Ms, 1, " ms"),
												<Budget key="budget" left={t.budgetRemaining} />,
												<span
													key="burn"
													className={textTone(burnTone(t.burnRate))}
												>
													{fmt(t.burnRate, 1, "×")}
												</span>,
											],
										])}
									/>
								</Panel>
								<Panel title="Isolation field" code="T-MAP" glow>
									<Field
										center="Control plane"
										centerSub={`${rows.length} tenants under watch`}
										nodes={rows.map((t) => ({
											name: t.name,
											tone: tenantTone(t),
											sub: `${t.tier} · ${fmt(t.p95Ms, 1, "ms")}`,
										}))}
										active
									/>
								</Panel>
							</div>
						</>
					);
				}}
			</Status>
		</>
	);
}

const fields: [keyof Amount, string, string][] = [
	["cpuMillis", "CPU", "m"],
	["memoryMiB", "Memory", "Mi"],
	["pods", "Pods", ""],
];

function Gauge({
	label,
	used,
	total,
	gate,
	unit,
}: {
	label: string;
	used: number;
	total: number;
	gate: number;
	unit: string;
}) {
	const percent = total ? (used / total) * 100 : 0;
	const tone: Tone =
		percent > gate ? "crit" : percent > gate * 0.8 ? "warn" : "accent";
	return (
		<div className="space-y-3">
			<div className="flex items-baseline justify-between gap-3">
				<span className="label text-text-2">{label}</span>
				<span className="font-mono text-[11px] text-text-3">
					{used}
					{unit} / {total}
					{unit}
				</span>
			</div>
			<div
				className={`font-mono text-4xl font-medium tabular-nums ${textTone(tone)}`}
			>
				{fmt(percent, 0)}
				<span className="ml-1 text-sm text-text-3">%</span>
			</div>
			<Meter percent={percent} tone={tone} marker={gate} thick />
			<div className="relative h-3 font-mono text-[10px] text-text-3">
				<span className="absolute left-0">0</span>
				<span
					className="absolute -translate-x-1/2 text-text-2"
					style={{ left: `${gate}%` }}
				>
					GATE
				</span>
				<span className="absolute right-0">100</span>
			</div>
		</div>
	);
}

function UseBar({
	used,
	requested,
	unit,
}: {
	used: number | null;
	requested: number | null;
	unit: string;
}) {
	if (used === null || requested === null) return "–";
	const p = requested ? (used / requested) * 100 : 0;
	return (
		<div className="flex items-center justify-end gap-3">
			<span className="text-text">{fmt(used, 0, unit)}</span>
			<span className="text-text-3">/ {fmt(requested, 0, unit)}</span>
			<div className="w-16">
				<Meter percent={p} tone={p > 90 ? "warn" : "accent"} />
			</div>
		</div>
	);
}

const usedOf: Partial<
	Record<keyof Amount, (t: CapacityView["tenants"][number]) => number | null>
> = {
	cpuMillis: (t) => t.cpuUsedMillis,
	memoryMiB: (t) => t.memoryUsedMib,
};

function towersOf(c: CapacityView): { towers: Tower[]; split: boolean } {
	const split = c.tenants.length > 0 && c.tenants.every((t) => t.quota);
	const towers = fields.map(([key, label, unit]) => {
		const used = usedOf[key];
		const sum = (f: (t: CapacityView["tenants"][number]) => number | null) =>
			c.tenants.reduce<number | null>((s, t) => {
				const v = f(t);
				return v === null ? s : (s ?? 0) + v;
			}, null);
		return {
			label,
			unit,
			capacity: c.capacity[key],
			slabs: split
				? c.tenants.map((t) => ({
						tenant: t.name,
						amount: t.quota?.[key] ?? 0,
						used: used ? used(t) : null,
					}))
				: [
						{
							tenant: "committed",
							amount: c.committed[key],
							used: used ? sum(used) : null,
						},
					],
		};
	});
	return { towers, split };
}

function Stack({ c }: { c: CapacityView }) {
	const { towers, split } = towersOf(c);
	const names = split ? c.tenants.map((t) => t.name) : ["committed"];
	return (
		<Panel
			title="Quota stack · live"
			code="C-3D"
			glow
			aside={
				<span className="hidden font-mono text-[10px] text-text-3 sm:inline">
					DRAG TO ORBIT
				</span>
			}
		>
			<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_260px]">
				<Stack3D towers={towers} gate={c.gatePercent} />
				<div className="space-y-4 self-center">
					<p className="text-[12px] leading-relaxed text-text-2">
						Each tower is the whole cluster. The layers are what tenants have
						been promised; the bright core inside each layer is what they are
						using right now. Particles rise faster through busier layers.
					</p>
					<ul className="space-y-2">
						{names.map((n, i) => {
							const t = c.tenants.find((x) => x.name === n);
							return (
								<li
									key={n}
									className="flex items-center justify-between gap-3 border border-line-2 bg-field px-3 py-2"
								>
									<span className="flex items-center gap-2 font-mono text-[12px] text-text">
										<span
											className="size-2.5"
											style={{
												background: tenantColors[i % tenantColors.length],
											}}
										/>
										{n}
									</span>
									<span className="font-mono text-[10px] text-text-3">
										{t
											? `${fmt(t.cpuUsedMillis, 0, "m")} · ${fmt(t.memoryUsedMib, 0, " Mi")}`
											: "all tenants"}
									</span>
								</li>
							);
						})}
					</ul>
					{!split && (
						<p className="font-mono text-[10px] leading-relaxed text-warn">
							This control plane does not report per-tenant quota yet, so the
							layers are merged. Update it to see one layer per tenant.
						</p>
					)}
				</div>
			</div>
		</Panel>
	);
}

export function Capacity() {
	return (
		<>
			<PageHead
				code="SYS/04"
				title="Capacity"
				sub="Committed is the sum of every tenant's quota: what has been promised. New tenants are refused once a commitment would cross the gate."
			/>
			<Status query={useCapacity()}>
				{(c) => (
					<>
						<Panel
							title={`Committed quota · gate ${c.gatePercent}%`}
							code="C-GATE"
							glow
						>
							<div className="grid gap-8 md:grid-cols-3">
								{fields.map(([key, label, unit]) => (
									<Gauge
										key={key}
										label={label}
										unit={unit}
										used={c.committed[key]}
										total={c.capacity[key]}
										gate={c.gatePercent}
									/>
								))}
							</div>
						</Panel>
						<Stack c={c} />
						<div className="grid gap-6 lg:grid-cols-[320px_minmax(0,1fr)]">
							<Panel title="Still fits" code="C-FIT">
								<div className="grid gap-3">
									{Object.entries(c.fits).map(([tier, n]) => (
										<div
											key={tier}
											className="flex items-center justify-between border border-line-2 bg-field px-4 py-3"
										>
											<span className="label text-text-2">{tier}</span>
											<span className="font-mono text-2xl text-accent tabular-nums">
												{n}
												<span className="ml-1 text-xs text-text-3">more</span>
											</span>
										</div>
									))}
								</div>
							</Panel>
							<Panel title="Used vs requested" code="C-USE" flush>
								<Table
									head={["Tenant", "Tier", "CPU", "Memory"]}
									align={["l", "l", "r", "r"]}
									rows={c.tenants.map((t) => [
										t.name,
										[
											<span
												key="n"
												className="font-sans text-[13px] font-medium text-text"
											>
												{t.name}
											</span>,
											<Chip key="tier">{t.tier}</Chip>,
											<UseBar
												key="cpu"
												used={t.cpuUsedMillis}
												requested={t.cpuRequestedMillis}
												unit="m"
											/>,
											<UseBar
												key="mem"
												used={t.memoryUsedMib}
												requested={t.memoryRequestedMib}
												unit=" Mi"
											/>,
										],
									])}
								/>
							</Panel>
						</div>
					</>
				)}
			</Status>
		</>
	);
}
