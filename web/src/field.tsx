import { useId } from "react";
import type { Tone } from "./ui";

export type Node = { name: string; tone: Tone; sub: string };

const colour: Record<Tone, string> = {
	accent: "#00afae",
	warn: "#e2a33c",
	crit: "#ee5b45",
	muted: "#6b7a77",
	plain: "#f2f3ee",
};

/**
 * A radial map: one node in the centre, the rest on an orbit. Lines carry the
 * tone of the node they reach, so an impacted bystander lights up red.
 */
export function Field({
	center,
	centerSub,
	centerTone = "accent",
	nodes,
	active,
}: {
	center: string;
	centerSub: string;
	centerTone?: Tone;
	nodes: Node[];
	active?: boolean;
}) {
	const glow = `glow${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
	const c = 200;
	const orbit = 138;
	const placed = nodes.map((n, i) => {
		const a = -Math.PI / 2 + (i * 2 * Math.PI) / Math.max(nodes.length, 1);
		return { ...n, x: c + orbit * Math.cos(a), y: c + orbit * Math.sin(a), a };
	});
	const core = colour[centerTone];
	return (
		<svg
			viewBox="0 0 400 400"
			className="mx-auto block w-full max-w-[420px]"
			role="img"
			aria-label={`${center} with ${nodes.map((n) => `${n.name} ${n.sub}`).join(", ")}`}
		>
			<defs>
				<radialGradient id={glow}>
					<stop offset="0%" stopColor={core} stopOpacity="0.35" />
					<stop offset="100%" stopColor={core} stopOpacity="0" />
				</radialGradient>
			</defs>
			<g fill="none" stroke="#3a4b4a">
				{[50, 92, orbit, 182].map((r, i) => (
					<circle
						key={r}
						cx={c}
						cy={c}
						r={r}
						strokeOpacity={i === 2 ? 0.9 : 0.45}
						strokeDasharray={i % 2 ? "2 5" : undefined}
					/>
				))}
				<line x1={c - 196} y1={c} x2={c + 196} y2={c} strokeOpacity="0.3" />
				<line x1={c} y1={c - 196} x2={c} y2={c + 196} strokeOpacity="0.3" />
			</g>
			<g
				className="spin-slow"
				fill="none"
				stroke="#00afae"
				strokeOpacity="0.35"
			>
				<circle cx={c} cy={c} r="182" strokeDasharray="1 12" strokeWidth="2" />
				<path d={`M ${c} ${c - 182} A 182 182 0 0 1 ${c + 182} ${c}`} />
			</g>
			<g className="spin-rev" fill="none" stroke="#00afae" strokeOpacity="0.22">
				<polygon
					points={Array.from({ length: 6 }, (_, i) => {
						const a = (i * Math.PI) / 3;
						return `${c + 92 * Math.cos(a)},${c + 92 * Math.sin(a)}`;
					}).join(" ")}
				/>
			</g>
			{[0, 45, 90, 135, 180, 225, 270, 315].map((deg) => {
				const a = (deg * Math.PI) / 180;
				return (
					<line
						key={deg}
						x1={c + 186 * Math.cos(a)}
						y1={c + 186 * Math.sin(a)}
						x2={c + 194 * Math.cos(a)}
						y2={c + 194 * Math.sin(a)}
						stroke="#6b7a77"
					/>
				);
			})}

			{placed.map((n) => (
				<line
					key={`l-${n.name}`}
					x1={c}
					y1={c}
					x2={n.x}
					y2={n.y}
					stroke={colour[n.tone]}
					strokeOpacity={n.tone === "crit" ? 0.85 : 0.35}
					strokeDasharray={n.tone === "crit" ? undefined : "3 4"}
				/>
			))}

			<circle cx={c} cy={c} r="70" fill={`url(#${glow})`} />
			{active && (
				<>
					<circle
						cx={c}
						cy={c}
						r="24"
						fill="none"
						stroke={core}
						className="pulse-ring"
					/>
					<circle
						cx={c}
						cy={c}
						r="24"
						fill="none"
						stroke={core}
						className="pulse-ring"
						style={{ animationDelay: "1.2s" }}
					/>
				</>
			)}
			<rect
				x={c - 9}
				y={c - 9}
				width="18"
				height="18"
				fill="#070b0b"
				stroke={core}
				transform={`rotate(45 ${c} ${c})`}
			/>
			<rect x={c - 3} y={c - 3} width="6" height="6" fill={core} />
			<text
				x={c}
				y={c + 34}
				textAnchor="middle"
				fill="#f2f3ee"
				fontFamily="var(--font-sans)"
				fontSize="12"
				fontWeight="600"
				letterSpacing="2"
			>
				{center.toUpperCase()}
			</text>
			<text
				x={c}
				y={c + 48}
				textAnchor="middle"
				fill="#6b7a77"
				fontFamily="var(--font-mono)"
				fontSize="10"
			>
				{centerSub}
			</text>

			{placed.map((n) => {
				const below = Math.sin(n.a) > -0.3;
				return (
					<g key={n.name}>
						<circle
							cx={n.x}
							cy={n.y}
							r="14"
							fill={colour[n.tone]}
							fillOpacity="0.08"
						/>
						<circle
							cx={n.x}
							cy={n.y}
							r="7"
							fill="#0c1212"
							stroke={colour[n.tone]}
							strokeWidth="1.5"
						/>
						<circle cx={n.x} cy={n.y} r="2.5" fill={colour[n.tone]} />
						<text
							x={n.x}
							y={n.y + (below ? 28 : -30)}
							textAnchor="middle"
							fill="#f2f3ee"
							fontFamily="var(--font-mono)"
							fontSize="12"
						>
							{n.name}
						</text>
						<text
							x={n.x}
							y={n.y + (below ? 40 : -18)}
							textAnchor="middle"
							fill={colour[n.tone]}
							fontFamily="var(--font-mono)"
							fontSize="10"
						>
							{n.sub}
						</text>
					</g>
				);
			})}
		</svg>
	);
}
