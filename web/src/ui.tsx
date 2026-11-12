import type { ButtonHTMLAttributes, ReactNode } from "react";

export const cn = (...classes: (string | false | null | undefined)[]) =>
	classes.filter(Boolean).join(" ");

export const fmt = (v: number | null | undefined, digits = 0, unit = "") =>
	v === null || v === undefined || Number.isNaN(v)
		? "–"
		: `${Number(v.toFixed(digits))}${unit}`;

export type Tone = "accent" | "warn" | "crit" | "muted" | "plain";

const toneText: Record<Tone, string> = {
	accent: "text-accent",
	warn: "text-warn",
	crit: "text-crit",
	muted: "text-text-3",
	plain: "text-text",
};

const toneChip: Record<Tone, string> = {
	accent: "text-accent border-accent-line bg-accent-dim",
	warn: "text-warn border-warn/45 bg-warn-dim",
	crit: "text-crit border-crit/45 bg-crit-dim",
	muted: "text-text-3 border-line-2 bg-field",
	plain: "text-text-2 border-line-2 bg-panel",
};

const toneFill: Record<Tone, string> = {
	accent: "bg-accent",
	warn: "bg-warn",
	crit: "bg-crit",
	muted: "bg-text-3",
	plain: "bg-text",
};

const toneGlow: Record<Tone, string> = {
	accent: "0 0 6px rgb(0 175 174 / 0.3)",
	warn: "0 0 6px rgb(226 163 60 / 0.3)",
	crit: "0 0 6px rgb(238 91 69 / 0.35)",
	muted: "none",
	plain: "none",
};

export const textTone = (t: Tone) => toneText[t];

export function Chip({
	tone = "plain",
	children,
	title,
	dot,
}: {
	tone?: Tone;
	children: ReactNode;
	title?: string;
	dot?: boolean;
}) {
	return (
		<span
			title={title}
			className={cn(
				"inline-flex h-[22px] items-center gap-1.5 border px-2 font-mono text-[10px] font-medium tracking-wide whitespace-nowrap",
				toneChip[tone],
			)}
		>
			{dot && <Dot tone={tone} />}
			{children}
		</span>
	);
}

export function Dot({ tone, live }: { tone: Tone; live?: boolean }) {
	return (
		<span className="relative inline-flex size-1.5 shrink-0">
			{live && (
				<span
					className={cn(
						"pulse-ring absolute inset-0 rounded-full",
						toneFill[tone],
					)}
				/>
			)}
			<span className={cn("relative size-1.5 rounded-full", toneFill[tone])} />
		</span>
	);
}

const buttonVariant = {
	primary:
		"bg-accent text-accent-ink border-accent hover:bg-accent/85 shadow-[0_0_24px_-6px_var(--color-accent)]",
	danger:
		"bg-crit-dim text-crit border-crit/60 hover:bg-crit hover:text-bg shadow-[0_0_28px_-8px_var(--color-crit)]",
	ghost:
		"bg-panel/80 text-text-2 border-line-2 hover:text-text hover:border-line-3",
};

export const buttonClass = (
	variant: keyof typeof buttonVariant = "ghost",
	className?: string,
) =>
	cn(
		"label inline-flex h-9 cursor-pointer items-center justify-center gap-2 border px-4 transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-40",
		buttonVariant[variant],
		className,
	);

export function Button({
	variant = "ghost",
	className,
	...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
	variant?: keyof typeof buttonVariant;
}) {
	return (
		<button
			type="button"
			{...props}
			className={buttonClass(variant, className)}
		/>
	);
}

export function Corners({ tone = "line" }: { tone?: "line" | "accent" }) {
	const c = tone === "accent" ? "border-accent" : "border-line-3";
	const base = cn("pointer-events-none absolute size-2.5", c);
	return (
		<>
			<span className={cn(base, "-top-px -left-px border-t border-l")} />
			<span className={cn(base, "-top-px -right-px border-t border-r")} />
			<span className={cn(base, "-bottom-px -left-px border-b border-l")} />
			<span className={cn(base, "-right-px -bottom-px border-r border-b")} />
		</>
	);
}

export function Panel({
	title,
	code,
	aside,
	children,
	className,
	flush,
	glow,
}: {
	title?: string;
	code?: string;
	aside?: ReactNode;
	children: ReactNode;
	className?: string;
	flush?: boolean;
	glow?: boolean;
}) {
	return (
		<section
			className={cn(
				"rise relative min-w-0 border border-line bg-panel/85 backdrop-blur-sm",
				glow && "shadow-[0_0_60px_-30px_var(--color-accent)]",
				className,
			)}
		>
			<Corners tone={glow ? "accent" : "line"} />
			{title && (
				<header className="flex items-center gap-3 border-b border-line px-4 py-2.5">
					{code && (
						<span className="shrink-0 font-mono text-[10px] whitespace-nowrap text-accent">
							{code}
						</span>
					)}
					<h2 className="label min-w-0 leading-snug text-text-2">{title}</h2>
					<span className="h-px min-w-3 flex-1 bg-line" />
					{aside}
				</header>
			)}
			<div className={flush ? "" : "p-4"}>{children}</div>
		</section>
	);
}

export function Stat({
	label,
	value,
	unit,
	tone = "plain",
	hint,
}: {
	label: string;
	value: string;
	unit?: string;
	tone?: Tone;
	hint?: string;
}) {
	return (
		<div className="rise relative min-w-0 border border-line bg-panel/85 p-3 backdrop-blur-sm sm:p-4">
			<Corners />
			<div className="label text-text-3">{label}</div>
			<div className="mt-3 flex items-baseline gap-1.5">
				<span
					className={cn(
						"font-mono text-3xl font-medium tracking-tight tabular-nums",
						toneText[tone],
					)}
				>
					{value}
				</span>
				{unit && <span className="font-mono text-xs text-text-3">{unit}</span>}
			</div>
			{hint && (
				<div className="mt-2 font-mono text-[10px] text-text-3">{hint}</div>
			)}
			<span
				className={cn(
					"absolute right-0 bottom-0 left-0 h-px opacity-60",
					toneFill[tone === "plain" ? "accent" : tone],
				)}
			/>
		</div>
	);
}

export function Meter({
	percent,
	tone = "accent",
	marker,
	thick,
}: {
	percent: number;
	tone?: Tone;
	marker?: number;
	thick?: boolean;
}) {
	const p = Math.max(0, Math.min(100, percent));
	return (
		<div
			className={cn("relative w-full bg-line-2", thick ? "h-1.5" : "h-0.5")}
			aria-hidden="true"
		>
			<div
				className={cn(
					"h-full opacity-80 transition-[width] duration-700 ease-out",
					toneFill[tone],
				)}
				style={{ width: `${p}%`, boxShadow: toneGlow[tone] }}
			/>
			{marker !== undefined && (
				<div
					className="absolute -top-1.5 -bottom-1.5 w-px bg-text"
					style={{ left: `${marker}%` }}
				/>
			)}
		</div>
	);
}

export function Segmented({
	label,
	options,
	value,
	onChange,
}: {
	label: string;
	options: string[];
	value: string;
	onChange: (v: string) => void;
}) {
	return (
		<fieldset className="min-w-0 space-y-2">
			<legend className="label mb-2 text-text-3">{label}</legend>
			<div className="flex flex-wrap gap-px border border-line-2 bg-line-2">
				{options.map((o) => (
					<button
						key={o}
						type="button"
						aria-pressed={o === value}
						onClick={() => onChange(o)}
						className={cn(
							"flex-[1_0_auto] cursor-pointer px-3 py-2 font-mono text-[11px] whitespace-nowrap transition-colors",
							o === value
								? "bg-accent-dim text-text shadow-[inset_0_-2px_0_var(--color-accent)]"
								: "bg-panel text-text-2 hover:bg-hover hover:text-text",
						)}
					>
						{o}
					</button>
				))}
			</div>
		</fieldset>
	);
}

export function Table({
	head,
	rows,
	align,
}: {
	head: string[];
	rows: [string, ReactNode[]][];
	align?: ("l" | "r")[];
}) {
	return (
		<>
			{/* Phones: one card per row, the first cell as its title. */}
			<ul className="divide-y divide-line sm:hidden">
				{rows.map(([key, cells]) => (
					<li key={key} className="space-y-3 px-4 py-3.5">
						<div className="min-w-0">{cells[0]}</div>
						<dl className="grid grid-cols-2 gap-x-4 gap-y-2.5">
							{cells.slice(1).map((c, i) => (
								<div key={head[i + 1]} className="min-w-0">
									<dt className="label text-[9px] text-text-3">
										{head[i + 1]}
									</dt>
									<dd className="mt-1 truncate font-mono text-xs text-text-2 tabular-nums [&>div]:justify-start">
										{c}
									</dd>
								</div>
							))}
						</dl>
					</li>
				))}
			</ul>
			<div className="hidden overflow-x-auto sm:block">
				<TableGrid head={head} rows={rows} align={align} />
			</div>
		</>
	);
}

function TableGrid({
	head,
	rows,
	align,
}: {
	head: string[];
	rows: [string, ReactNode[]][];
	align?: ("l" | "r")[];
}) {
	return (
		<table className="w-full border-collapse text-left">
			<thead>
				<tr className="border-b border-line">
					{head.map((h, i) => (
						<th
							key={h}
							className={cn(
								"label px-4 py-3 font-semibold whitespace-nowrap text-text-3",
								align?.[i] === "r" && "text-right",
							)}
						>
							{h}
						</th>
					))}
				</tr>
			</thead>
			<tbody>
				{rows.map(([key, cells]) => (
					<tr
						key={key}
						className="group border-b border-line/70 transition-colors last:border-0 hover:bg-hover/60"
					>
						{cells.map((c, i) => (
							<td
								key={head[i]}
								className={cn(
									"px-4 py-3 font-mono text-xs whitespace-nowrap text-text-2 tabular-nums",
									align?.[i] === "r" && "text-right",
								)}
							>
								{c}
							</td>
						))}
					</tr>
				))}
			</tbody>
		</table>
	);
}

export function Empty({ title, sub }: { title: string; sub: string }) {
	return (
		<div className="flex flex-col items-center gap-4 px-6 py-14 text-center">
			<svg viewBox="0 0 120 120" className="size-24" aria-hidden="true">
				<g className="spin-slow" fill="none" stroke="var(--color-line-3)">
					<circle cx="60" cy="60" r="54" strokeDasharray="2 6" />
					<polygon points="60,14 100,37 100,83 60,106 20,83 20,37" />
				</g>
				<g className="spin-rev" fill="none" stroke="var(--color-accent-line)">
					<polygon points="60,32 84,46 84,74 60,88 36,74 36,46" />
				</g>
				<circle cx="60" cy="60" r="3" fill="var(--color-accent)" />
			</svg>
			<div className="label text-text">{title}</div>
			<p className="max-w-sm font-mono text-[11px] text-text-3">{sub}</p>
		</div>
	);
}

export function Status<T>({
	query,
	children,
}: {
	query: { data?: T; error: Error | null };
	children: (data: T) => ReactNode;
}) {
	if (query.error && !query.data)
		return (
			<Panel title="Signal lost" code="ERR">
				<p className="font-mono text-xs text-crit">{query.error.message}</p>
			</Panel>
		);
	if (!query.data)
		return (
			<div className="flex items-center gap-3 py-10">
				<div className="relative h-px w-40 overflow-hidden bg-line-2">
					<div className="scan absolute inset-y-0 w-1/4 bg-accent" />
				</div>
				<span className="label text-text-3">Acquiring telemetry</span>
			</div>
		);
	return children(query.data);
}

export function PageHead({
	code,
	title,
	sub,
	aside,
}: {
	code: string;
	title: string;
	sub: string;
	aside?: ReactNode;
}) {
	return (
		<div className="rise flex flex-wrap items-end gap-4">
			<div className="min-w-0 flex-1">
				<div className="flex items-center gap-3">
					<span className="font-mono text-[10px] text-accent">{code}</span>
					<span className="h-px w-10 bg-accent-line" />
				</div>
				<h1 className="mt-2 text-3xl font-semibold tracking-tight [font-stretch:115%] sm:text-4xl">
					{title}
				</h1>
				<p className="mt-2 max-w-2xl text-sm text-text-2">{sub}</p>
			</div>
			{aside}
		</div>
	);
}
