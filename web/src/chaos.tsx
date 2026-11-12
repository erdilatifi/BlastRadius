import { useEffect, useState } from "react";
import {
	type Experiment,
	type Run,
	useExperiments,
	useRuns,
	useStartRun,
	useTenants,
} from "./api";
import { Field, type Node } from "./field";
import {
	Button,
	Chip,
	cn,
	Dot,
	Empty,
	fmt,
	PageHead,
	Panel,
	Segmented,
	Stat,
	Status,
	Table,
	type Tone,
	textTone,
} from "./ui";

const phases = ["baseline", "inject", "recovery", "scoring", "done"] as const;

const roleTone: Record<string, Tone> = {
	target: "plain",
	clean: "accent",
	impacted: "crit",
};

const clock = (s: number) =>
	`${String(Math.floor(s / 60)).padStart(2, "0")}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

function useNow() {
	const [now, setNow] = useState(() => Date.now() / 1000);
	useEffect(() => {
		const id = setInterval(() => setNow(Date.now() / 1000), 1000);
		return () => clearInterval(id);
	}, []);
	return now;
}

function Stepper({ run }: { run: Run }) {
	const at = phases.indexOf(run.phase as (typeof phases)[number]);
	const failed = run.phase === "failed";
	return (
		<ol className="grid grid-cols-5 gap-px border border-line-2 bg-line-2">
			{phases.map((p, i) => {
				const done = i < at || run.phase === "done";
				const now = i === at && run.phase !== "done";
				return (
					<li
						key={p}
						className={cn(
							"relative min-w-0 overflow-hidden px-1.5 py-2.5 sm:px-3 sm:py-3",
							now
								? p === "inject"
									? "bg-crit-dim"
									: "bg-accent-dim"
								: "bg-panel",
						)}
					>
						<div className="font-mono text-[10px] text-text-3">0{i + 1}</div>
						<div
							className={cn(
								"mt-1.5 truncate font-sans text-[8.5px] leading-none font-semibold tracking-normal uppercase sm:text-[10px] sm:[font-stretch:110%] sm:tracking-[0.16em]",
								failed
									? "text-crit"
									: now
										? p === "inject"
											? "text-crit"
											: "text-text"
										: done
											? "text-accent"
											: "text-text-3",
							)}
						>
							{p}
						</div>
						<span
							className={cn(
								"absolute inset-x-0 bottom-0 h-0.5",
								done ? "bg-accent" : now ? "bg-line-3" : "bg-transparent",
							)}
						/>
						{now && (
							<span
								className={cn(
									"scan absolute bottom-0 left-0 h-0.5 w-1/3",
									p === "inject" ? "bg-crit" : "bg-accent",
								)}
							/>
						)}
					</li>
				);
			})}
		</ol>
	);
}

function Live({ run, experiment }: { run: Run; experiment?: Experiment }) {
	const now = useNow();
	return (
		<Panel
			title={`Live run · ${run.experiment} → ${run.target}`}
			code="X-LIVE"
			glow
			aside={
				<Chip tone={run.phase === "inject" ? "crit" : "accent"} dot>
					{run.phase.toUpperCase()}
				</Chip>
			}
		>
			<div className="space-y-5">
				<div className="flex flex-wrap items-end justify-between gap-4">
					<div>
						<div className="label text-text-3">Elapsed</div>
						<div className="mt-2 font-mono text-5xl font-medium tabular-nums">
							T+{clock(Math.max(0, now - run.startedAt))}
						</div>
					</div>
					<p className="max-w-md text-sm text-text-2">
						<span className="label mr-2 text-text-3">Hypothesis</span>
						{experiment?.hypothesis ?? "–"}
					</p>
				</div>
				<Stepper run={run} />
				<p className="font-mono text-[11px] text-text-3">
					Baseline → inject {experiment ? `${experiment.injectSeconds}s` : ""} →
					recovery → score every bystander. A bystander that moves counts toward
					the blast radius.
				</p>
			</div>
		</Panel>
	);
}

function Launcher({ experiments }: { experiments: string[] }) {
	const tenants = useTenants().data?.map((t) => t.name) ?? [];
	const [experiment, setExperiment] = useState("");
	const [tenant, setTenant] = useState("");
	const start = useStartRun();
	const chosen = [experiment || experiments[0], tenant || tenants[0]];
	return (
		<Panel title="Arm an experiment" code="X-ARM">
			<form
				className="space-y-5"
				onSubmit={(e) => {
					e.preventDefault();
					start.mutate({ experiment: chosen[0], tenant: chosen[1] });
				}}
			>
				<Segmented
					label="Fault"
					options={experiments}
					value={chosen[0]}
					onChange={setExperiment}
				/>
				<Segmented
					label="Target tenant"
					options={tenants}
					value={chosen[1] ?? ""}
					onChange={setTenant}
				/>
				<div className="flex flex-wrap items-center gap-4 border-t border-line pt-5">
					<Button
						type="submit"
						variant="danger"
						disabled={start.isPending || !chosen[1]}
					>
						<span className="size-1.5 bg-current" />
						{start.isPending ? "Arming…" : "Inject fault"}
					</Button>
					<span className="font-mono text-[11px] text-text-3">
						{chosen[0]} → {chosen[1] ?? "–"} · only namespaces that opt in can
						be targeted
					</span>
					{start.error && (
						<span className="font-mono text-[11px] text-crit">
							{start.error.message}
						</span>
					)}
				</div>
			</form>
		</Panel>
	);
}

const radiusOf = (r: Run) =>
	r.phase === "failed" ? (
		<span className="text-crit" title={r.error ?? ""}>
			failed
		</span>
	) : r.score?.invalid ? (
		<span className="text-warn" title={r.score.invalid}>
			invalid
		</span>
	) : (
		<span
			className={(r.score?.blastRadius ?? 0) > 0 ? "text-crit" : "text-accent"}
		>
			{fmt(r.score?.blastRadius ?? null)}
		</span>
	);

function Matrix({ runs }: { runs: Run[] }) {
	const tenants = [
		...new Set(
			runs.flatMap((r) => r.score?.tenants.map((d) => d.tenant) ?? []),
		),
	].sort();
	return (
		<Table
			head={[
				"Run",
				"Started (UTC)",
				"Blast radius",
				"Detect",
				"Recover",
				...tenants,
			]}
			align={["l", "l", "r", "r", "r", ...tenants.map(() => "l" as const)]}
			rows={[...runs].reverse().map((r) => [
				r.id,
				[
					<span key="run" className="font-sans text-[13px] text-text">
						{r.experiment} <span className="text-text-3">→</span> {r.target}
					</span>,
					new Date(r.startedAt * 1000)
						.toISOString()
						.slice(5, 16)
						.replace("T", " "),
					radiusOf(r),
					fmt(r.detectSeconds, 0, " s"),
					fmt(r.recoverSeconds, 0, " s"),
					...tenants.map((t) => {
						const d = r.score?.tenants.find((x) => x.tenant === t);
						return d ? (
							<Chip
								key={t}
								tone={roleTone[d.role]}
								title={d.reasons.join("; ")}
							>
								{d.role}
							</Chip>
						) : (
							"–"
						);
					}),
				],
			])}
		/>
	);
}

function Radius({ run, live }: { run?: Run; live?: Run }) {
	const tenants = useTenants().data ?? [];
	const shown = live ?? run;
	if (!shown)
		return (
			<Empty
				title="No runs yet"
				sub="Arm an experiment to see how far a fault travels."
			/>
		);
	const nodes: Node[] = live
		? tenants
				.filter((t) => t.name !== live.target)
				.map((t) => ({ name: t.name, tone: "muted", sub: "watching" }))
		: (shown.score?.tenants ?? [])
				.filter((d) => d.role !== "target")
				.map((d) => ({
					name: d.tenant,
					tone: roleTone[d.role],
					sub:
						d.inject && d.baseline
							? `p95 ${fmt(d.baseline.p95Ms, 1)}→${fmt(d.inject.p95Ms, 1)}`
							: d.role,
				}));
	return (
		<Field
			center={shown.target}
			centerSub={
				live
					? `${live.experiment} · ${live.phase}`
					: `${shown.experiment} · target`
			}
			centerTone={live ? "crit" : "plain"}
			nodes={nodes}
			active={Boolean(live)}
		/>
	);
}

export function Chaos() {
	const runs = useRuns().data ?? [];
	const live = runs.find((r) => !["done", "failed"].includes(r.phase));
	const finished = runs.filter((r) => r.score || r.phase === "failed");
	const scored = finished.filter(
		(r) => r.score && !r.score.invalid && r.phase !== "failed",
	);
	const last = scored.at(-1);
	const worst = Math.max(0, ...scored.map((r) => r.score?.blastRadius ?? 0));
	const recover = scored.map((r) => r.recoverSeconds).filter((v) => v !== null);
	return (
		<>
			<PageHead
				code="SYS/02"
				title="Chaos"
				sub="Inject one fault into one tenant, watch every other tenant, and count how many moved. Zero is the goal."
				aside={
					live ? (
						<Chip tone="crit" dot>
							<span className="blink">FAULT ACTIVE</span>
						</Chip>
					) : (
						<Chip tone="accent" dot>
							READY
						</Chip>
					)
				}
			/>
			<Status query={useExperiments()}>
				{(catalog) => (
					<>
						<div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
							<Stat
								label="Runs this session"
								value={String(finished.length)}
								hint="Kept in the control plane"
							/>
							<Stat
								label="Scored"
								value={`${scored.length}/${finished.length || 0}`}
								hint="Invalid runs are reported, not hidden"
							/>
							<Stat
								label="Worst blast radius"
								value={String(worst)}
								tone={worst > 0 ? "crit" : "accent"}
								hint="Bystanders that moved"
							/>
							<Stat
								label="Median recover"
								value={
									recover.length
										? fmt(
												[...recover].sort((a, b) => a - b)[
													Math.floor(recover.length / 2)
												],
											)
										: "–"
								}
								unit="s"
								hint="Fault removed → healthy"
							/>
						</div>
						<div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_440px]">
							<div className="min-w-0 space-y-6">
								{live ? (
									<Live run={live} experiment={catalog[live.experiment]} />
								) : (
									<Launcher experiments={Object.keys(catalog)} />
								)}
								<Panel title="Experiment catalogue" code="X-CAT" flush>
									<ul className="grid sm:grid-cols-2">
										{Object.entries(catalog).map(([name, e], i) => (
											<li
												key={name}
												className={cn(
													"space-y-2 border-line p-4 transition-colors hover:bg-hover/50",
													"border-b sm:odd:border-r",
												)}
											>
												<div className="flex items-center justify-between gap-2">
													<span className="font-mono text-[13px] text-text">
														<span className="mr-2 text-[10px] text-accent">
															E{String(i + 1).padStart(2, "0")}
														</span>
														{name}
													</span>
													<Chip tone="muted">{e.injectSeconds}s</Chip>
												</div>
												<p className="text-[12px] leading-relaxed text-text-2">
													{e.hypothesis}
												</p>
											</li>
										))}
									</ul>
								</Panel>
							</div>
							<Panel
								title="Blast field"
								code="X-MAP"
								glow
								aside={
									last && !live ? (
										<span
											className={cn(
												"font-mono text-[11px]",
												textTone(
													(last.score?.blastRadius ?? 0) > 0
														? "crit"
														: "accent",
												),
											)}
										>
											R = {last.score?.blastRadius}
										</span>
									) : undefined
								}
							>
								<Radius run={last} live={live} />
								<div className="mt-4 flex flex-wrap justify-center gap-4 font-mono text-[10px] text-text-3">
									<span className="flex items-center gap-1.5">
										<Dot tone="plain" /> target
									</span>
									<span className="flex items-center gap-1.5">
										<Dot tone="accent" /> clean
									</span>
									<span className="flex items-center gap-1.5">
										<Dot tone="crit" /> impacted
									</span>
								</div>
							</Panel>
						</div>
						<Panel title="Run ledger" code="X-LOG" flush>
							{finished.length ? (
								<Matrix runs={finished} />
							) : (
								<Empty
									title="Ledger empty"
									sub="Finished runs land here with a score per bystander."
								/>
							)}
						</Panel>
					</>
				)}
			</Status>
		</>
	);
}
