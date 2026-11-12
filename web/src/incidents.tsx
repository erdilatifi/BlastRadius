import { useState } from "react";
import { type Incident, useIncidents } from "./api";
import {
	buttonClass,
	Chip,
	cn,
	Dot,
	Empty,
	fmt,
	PageHead,
	Panel,
	Stat,
	Status,
} from "./ui";

const time = (at: number) =>
	new Date(at * 1000).toISOString().replace("T", " ").slice(0, 19);

function Row({
	i,
	open,
	onOpen,
}: {
	i: Incident;
	open: boolean;
	onOpen: () => void;
}) {
	const firing = !i.resolvedAt;
	return (
		<li>
			<button
				type="button"
				onClick={onOpen}
				aria-pressed={open}
				className={cn(
					"grid w-full cursor-pointer grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-4 border-b border-line px-4 py-3 text-left transition-colors",
					open ? "bg-accent-dim" : "hover:bg-hover/60",
				)}
			>
				<Dot tone={firing ? "crit" : "accent"} live={firing} />
				<span className="min-w-0">
					<span className="block truncate font-mono text-[13px] text-text">
						{i.alert}
					</span>
					<span className="mt-1 block font-mono text-[10px] text-text-3">
						{i.tenant} · {time(i.firedAt)} UTC
					</span>
				</span>
				<Chip tone={firing ? "crit" : "accent"}>
					{firing
						? "FIRING"
						: `${i.resolvedAt ? i.resolvedAt - i.firedAt : 0}s`}
				</Chip>
			</button>
		</li>
	);
}

function Detail({ i }: { i: Incident }) {
	return (
		<Panel
			title="Post-mortem draft"
			code={`I-${i.fingerprint.slice(0, 6).toUpperCase()}`}
			glow
			aside={
				<div className="flex gap-2">
					{i.pr && (
						<a href={i.pr} className={buttonClass("primary", "h-7 px-3")}>
							Draft PR
						</a>
					)}
					{i.fix?.startsWith("https://") && (
						<a href={i.fix} className={buttonClass("ghost", "h-7 px-3")}>
							Fix PR
						</a>
					)}
				</div>
			}
		>
			<div className="mb-4 grid grid-cols-3 gap-px border border-line-2 bg-line-2">
				{[
					["Tenant", i.tenant],
					["Detect", fmt(i.detectSeconds, 0, " s")],
					[
						"Fix",
						i.fix && !i.fix.startsWith("https://")
							? i.fix
							: i.fix
								? "PR opened"
								: "–",
					],
				].map(([k, v]) => (
					<div key={k} className="bg-field px-3 py-2.5">
						<div className="label text-text-3">{k}</div>
						<div className="mt-1.5 truncate font-mono text-xs text-text">
							{v}
						</div>
					</div>
				))}
			</div>
			<pre className="max-h-[560px] overflow-auto border border-line bg-field p-4 font-mono text-[11px] leading-relaxed whitespace-pre-wrap text-text-2">
				{i.markdown}
			</pre>
		</Panel>
	);
}

export function Incidents() {
	const [picked, setPicked] = useState<string | null>(null);
	return (
		<>
			<PageHead
				code="SYS/03"
				title="Incidents"
				sub="Burn-rate alerts become blameless post-mortem drafts full of evidence. Root cause stays blank: that judgement belongs to a human."
			/>
			<Status query={useIncidents()}>
				{(incidents) => {
					const firing = incidents.filter((i) => !i.resolvedAt).length;
					const detects = incidents
						.map((i) => i.detectSeconds)
						.filter((v) => v !== null);
					const open =
						incidents.find((i) => i.fingerprint === picked) ?? incidents[0];
					return (
						<>
							<div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
								<Stat
									label="Firing now"
									value={String(firing)}
									tone={firing ? "crit" : "accent"}
									hint="Burn-rate alerts"
								/>
								<Stat
									label="Drafts"
									value={String(incidents.length)}
									hint="Since the control plane started"
								/>
								<Stat
									label="Fastest detect"
									value={detects.length ? fmt(Math.min(...detects)) : "–"}
									unit="s"
									hint="Fault → alert"
								/>
								<Stat
									label="Root causes guessed"
									value="0"
									tone="accent"
									hint="By design"
								/>
							</div>
							{incidents.length === 0 ? (
								<Panel title="Alert stream" code="I-LOG">
									<Empty
										title="All quiet"
										sub="No burn-rate alert has fired. Run error-burst on the Chaos page to see the Scribe draft a post-mortem."
									/>
								</Panel>
							) : (
								<div className="grid gap-6 lg:grid-cols-[380px_minmax(0,1fr)]">
									<Panel title="Alert stream" code="I-LOG" flush>
										<ul>
											{incidents.map((i) => (
												<Row
													key={i.fingerprint}
													i={i}
													open={open?.fingerprint === i.fingerprint}
													onOpen={() => setPicked(i.fingerprint)}
												/>
											))}
										</ul>
									</Panel>
									{open && <Detail i={open} />}
								</div>
							)}
						</>
					);
				}}
			</Status>
		</>
	);
}
