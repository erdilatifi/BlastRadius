import { useQuery } from "@tanstack/react-query";

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
