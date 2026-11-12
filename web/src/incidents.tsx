import { useState } from "react";
import { type Incident, useIncidents } from "./api";
import { Status, show, Table } from "./pages";

const time = (at: number) =>
	new Date(at * 1000).toISOString().replace("T", " ").slice(0, 19);

export function Incidents() {
	const [open, setOpen] = useState<Incident | null>(null);
	return (
		<Status query={useIncidents()}>
			{(incidents) => (
				<div className="space-y-6">
					<Table
						head={[
							"Alert",
							"Tenant",
							"Fired (UTC)",
							"Detect",
							"Recover",
							"Post-mortem",
							"Fix",
						]}
						rows={incidents.map((i) => [
							i.fingerprint,
							[
								i.alert,
								i.tenant,
								time(i.firedAt),
								show(i.detectSeconds, " s"),
								i.resolvedAt ? `${i.resolvedAt - i.firedAt} s` : "firing",
								i.pr ? (
									<a key="pr" className="text-blue-700 underline" href={i.pr}>
										draft PR
									</a>
								) : (
									<button
										key="draft"
										type="button"
										className="text-blue-700 underline"
										onClick={() => setOpen(i)}
									>
										draft
									</button>
								),
								i.fix?.startsWith("https://") ? (
									<a key="fix" className="text-blue-700 underline" href={i.fix}>
										fix PR
									</a>
								) : (
									(i.fix ?? "–")
								),
							],
						])}
					/>
					{open && (
						<pre className="whitespace-pre-wrap rounded border bg-gray-50 p-4 text-xs">
							{open.markdown}
						</pre>
					)}
				</div>
			)}
		</Status>
	);
}
