import { useState } from "react";
import {
	type Run,
	useExperiments,
	useRuns,
	useStartRun,
	useTenants,
} from "./api";
import { Status, show, Table } from "./pages";

const phases = ["baseline", "inject", "recovery", "scoring", "done"];

const roleColour = {
	target: "bg-gray-200 text-gray-700",
	clean: "bg-green-100 text-green-800",
	impacted: "bg-red-100 text-red-800",
};

function Live({ run }: { run: Run }) {
	const at = phases.indexOf(run.phase);
	return (
		<section className="space-y-2">
			<h2 className="font-semibold">
				Running {run.experiment} on {run.target}
			</h2>
			<ol className="flex gap-2 text-sm">
				{phases.map((p, i) => (
					<li
						key={p}
						className={`rounded px-2 py-1 ${i < at ? "bg-blue-100" : i === at ? "bg-blue-600 text-white" : "bg-gray-100 text-gray-500"}`}
					>
						{p}
					</li>
				))}
			</ol>
		</section>
	);
}

function Launcher({ experiments }: { experiments: string[] }) {
	const tenants = useTenants().data?.map((t) => t.name) ?? [];
	const [experiment, setExperiment] = useState("");
	const [tenant, setTenant] = useState("");
	const start = useStartRun();
	const chosen = [experiment || experiments[0], tenant || tenants[0]];
	return (
		<form
			className="flex items-center gap-3 text-sm"
			onSubmit={(e) => {
				e.preventDefault();
				start.mutate({ experiment: chosen[0], tenant: chosen[1] });
			}}
		>
			<select
				className="rounded border px-2 py-1"
				value={chosen[0]}
				onChange={(e) => setExperiment(e.target.value)}
			>
				{experiments.map((x) => (
					<option key={x}>{x}</option>
				))}
			</select>
			<select
				className="rounded border px-2 py-1"
				value={chosen[1]}
				onChange={(e) => setTenant(e.target.value)}
			>
				{tenants.map((x) => (
					<option key={x}>{x}</option>
				))}
			</select>
			<button
				type="submit"
				className="rounded bg-red-600 px-3 py-1 text-white disabled:opacity-50"
				disabled={start.isPending || !chosen[1]}
			>
				Run
			</button>
			{start.error && (
				<span className="text-red-600">{start.error.message}</span>
			)}
		</form>
	);
}

function Matrix({ runs }: { runs: Run[] }) {
	const tenants = [
		...new Set(
			runs.flatMap((r) => r.score?.tenants.map((d) => d.tenant) ?? []),
		),
	].sort();
	const radius = (r: Run) =>
		r.phase === "failed"
			? `failed: ${r.error}`
			: r.score?.invalid
				? `invalid: ${r.score.invalid}`
				: show(r.score?.blastRadius ?? null);
	return (
		<Table
			head={["Run", "Blast radius", "Detect", "Recover", ...tenants]}
			rows={[...runs].reverse().map((r) => [
				r.id,
				[
					`${r.experiment} → ${r.target}`,
					radius(r),
					show(r.detectSeconds, " s"),
					show(r.recoverSeconds, " s"),
					...tenants.map((t) => {
						const d = r.score?.tenants.find((x) => x.tenant === t);
						return d ? (
							<span
								key={t}
								title={d.reasons.join("; ")}
								className={`rounded px-2 py-0.5 ${roleColour[d.role]}`}
							>
								{d.role}
							</span>
						) : (
							"–"
						);
					}),
				],
			])}
		/>
	);
}

export function Chaos() {
	const runs = useRuns().data ?? [];
	const live = runs.find((r) => !["done", "failed"].includes(r.phase));
	return (
		<Status query={useExperiments()}>
			{(catalog) => (
				<div className="space-y-8">
					{live ? (
						<Live run={live} />
					) : (
						<Launcher experiments={Object.keys(catalog)} />
					)}
					<Table
						head={["Experiment", "Inject", "Hypothesis"]}
						rows={Object.entries(catalog).map(([name, e]) => [
							name,
							[name, `${e.injectSeconds} s`, e.hypothesis],
						])}
					/>
					<Matrix runs={runs.filter((r) => r.score || r.phase === "failed")} />
				</div>
			)}
		</Status>
	);
}
