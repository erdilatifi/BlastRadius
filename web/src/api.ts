import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

export type TenantRow = {
	name: string;
	tier: string;
	sync: string;
	health: string;
	availability: number | null;
	target: number | null;
	p95Ms: number | null;
	budgetRemaining: number | null;
	burnRate: number | null;
};

export type Amount = { cpuMillis: number; memoryMiB: number; pods: number };

export type Usage = {
	name: string;
	tier: string;
	cpuUsedMillis: number | null;
	cpuRequestedMillis: number | null;
	memoryUsedMib: number | null;
	memoryRequestedMib: number | null;
};

export type CapacityView = {
	capacity: Amount;
	committed: Amount;
	gatePercent: number;
	fits: Record<string, number>;
	tenants: Usage[];
};

export type Experiment = { hypothesis: string; injectSeconds: number };

export type Window = {
	errorPp: number;
	p95Ms: number;
	restarts: number;
	unready: number;
};

export type Delta = {
	tenant: string;
	role: "target" | "clean" | "impacted";
	baseline: Window | null;
	inject: Window | null;
	reasons: string[];
};

export type Run = {
	id: string;
	experiment: string;
	target: string;
	phase: "baseline" | "inject" | "recovery" | "scoring" | "done" | "failed";
	startedAt: number;
	detectSeconds: number | null;
	recoverSeconds: number | null;
	score: {
		invalid: string | null;
		blastRadius: number | null;
		tenants: Delta[];
	} | null;
	error: string | null;
};

export type Incident = {
	fingerprint: string;
	alert: string;
	tenant: string;
	firedAt: number;
	resolvedAt: number | null;
	detectSeconds: number | null;
	markdown: string;
	pr: string | null;
	fix: string | null;
};

async function get<T>(path: string): Promise<T> {
	const response = await fetch(path);
	if (!response.ok)
		throw new Error(`${path}: ${response.status} ${await response.text()}`);
	return response.json();
}

export const useTenants = () =>
	useQuery({
		queryKey: ["tenants"],
		queryFn: () => get<TenantRow[]>("/api/tenants"),
	});

export const useCapacity = () =>
	useQuery({
		queryKey: ["capacity"],
		queryFn: () => get<CapacityView>("/api/capacity"),
	});

export const useExperiments = () =>
	useQuery({
		queryKey: ["experiments"],
		queryFn: () => get<Record<string, Experiment>>("/api/experiments"),
	});

export const useRuns = () =>
	useQuery({ queryKey: ["runs"], queryFn: () => get<Run[]>("/api/runs") });

export const useIncidents = () =>
	useQuery({
		queryKey: ["incidents"],
		queryFn: () => get<Incident[]>("/api/incidents"),
	});

export function useStartRun() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (body: { experiment: string; tenant: string }) => {
			const response = await fetch("/api/runs", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify(body),
			});
			if (!response.ok) throw new Error(await response.text());
			return (await response.json()) as Run;
		},
		onSuccess: () => client.invalidateQueries({ queryKey: ["runs"] }),
	});
}
