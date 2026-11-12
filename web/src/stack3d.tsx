import { useEffect, useRef } from "react";

// A small hand-rolled 3D renderer: no WebGL, no library. Each resource is a
// glass tower as tall as the cluster's capacity; each tenant's quota is a slab
// stacked inside it, and a brighter core inside the slab rises with what the
// tenant is actually using. Particles drift up through busy slabs.

export type Slab = { tenant: string; amount: number; used: number | null };

export type Tower = {
	label: string;
	unit: string;
	capacity: number;
	slabs: Slab[];
};

type V3 = [number, number, number];
type Face = { pts: V3[]; fill: string; stroke: string; depth: number };

export const tenantColors = [
	"#00AFAE",
	"#5FD8CE",
	"#1B7F86",
	"#9BE8DF",
	"#0D5C61",
];

const W = 1; // tower footprint
const H = 3.4; // tower height = 100 % capacity
const GAP = 1.9;

const hex = (c: string, a: number) => {
	const n = Number.parseInt(c.slice(1), 16);
	return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
};

function box(
	cx: number,
	cz: number,
	w: number,
	y0: number,
	y1: number,
	fill: string,
	stroke: string,
): { pts: V3[]; fill: string; stroke: string }[] {
	const h = w / 2;
	const [x0, x1, z0, z1] = [cx - h, cx + h, cz - h, cz + h];
	const quad = (...p: V3[]) => ({ pts: p, fill, stroke });
	return [
		quad([x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]),
		quad([x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]),
		quad([x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]),
		quad([x1, y0, z0], [x1, y0, z1], [x1, y1, z1], [x1, y1, z0]),
		quad([x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]),
		quad([x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]),
	];
}

type Mote = {
	tower: number;
	x: number;
	z: number;
	y: number;
	top: number;
	v: number;
};

export function Stack3D({ towers, gate }: { towers: Tower[]; gate: number }) {
	const canvas = useRef<HTMLCanvasElement>(null);
	const data = useRef({ towers, gate });
	data.current = { towers, gate };

	useEffect(() => {
		const el = canvas.current;
		const ctx = el?.getContext("2d");
		if (!el || !ctx) return;
		const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
		// Sway gently around a readable angle instead of spinning, so the
		// towers never line up behind each other unless the viewer drags them.
		let base = -0.38;
		let sway = 0;
		let yaw = base;
		let pitch = 0.42;
		let drag: { x: number; y: number; yaw: number; pitch: number } | null =
			null;
		let idle = 0;
		let frame = 0;
		let last = performance.now();
		let motes: Mote[] = [];
		let size = { w: 0, h: 0 };

		const resize = () => {
			const r = el.getBoundingClientRect();
			const dpr = Math.min(devicePixelRatio || 1, 2);
			size = { w: r.width, h: r.height };
			el.width = r.width * dpr;
			el.height = r.height * dpr;
			ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		};
		resize();
		const ro = new ResizeObserver(resize);
		ro.observe(el);

		const project = ([x, y, z]: V3): [number, number, number] => {
			const cy = Math.cos(yaw);
			const sy = Math.sin(yaw);
			const x1 = x * cy - z * sy;
			const z1 = x * sy + z * cy;
			const y0 = y - H / 2;
			const cp = Math.cos(pitch);
			const sp = Math.sin(pitch);
			const y2 = y0 * cp - z1 * sp;
			const z2 = y0 * sp + z1 * cp;
			const scale = Math.min(size.w / 6.8, size.h / 5.8);
			const persp = 24 / (24 + z2);
			return [
				size.w / 2 + x1 * scale * persp,
				size.h / 2 - 4 - y2 * scale * persp,
				z2,
			];
		};

		const draw = (dt: number) => {
			const { towers: ts, gate: g } = data.current;
			ctx.clearRect(0, 0, size.w, size.h);
			const n = ts.length;
			const xs = ts.map((_, i) => (i - (n - 1) / 2) * GAP);

			// Floor grid
			ctx.lineWidth = 1;
			const span = Math.max(3, (n * GAP) / 2 + 0.8);
			for (let i = -span; i <= span + 1e-6; i += 0.5) {
				const major = Math.abs(i % 1) < 1e-6;
				ctx.strokeStyle = major ? "rgba(58,75,74,0.55)" : "rgba(38,52,52,0.45)";
				for (const [a, b] of [
					[
						[i, 0, -1.6],
						[i, 0, 1.6],
					],
					...(Math.abs(i) <= 1.6
						? [
								[
									[-span, 0, i],
									[span, 0, i],
								],
							]
						: []),
				] as [V3, V3][]) {
					const p = project(a);
					const q = project(b);
					ctx.beginPath();
					ctx.moveTo(p[0], p[1]);
					ctx.lineTo(q[0], q[1]);
					ctx.stroke();
				}
			}

			const faces: Face[] = [];
			const push = (f: { pts: V3[]; fill: string; stroke: string }) => {
				const depth =
					f.pts.reduce((s, p) => s + project(p)[2], 0) / f.pts.length;
				faces.push({ ...f, depth });
			};

			ts.forEach((t, ti) => {
				const cx = xs[ti];
				const full =
					t.slabs.reduce((s, x) => s + x.amount, 0) / (t.capacity || 1);
				const over = full * 100 > g;
				// Glass tower
				for (const f of box(
					cx,
					0,
					W,
					0,
					H,
					"rgba(16,24,24,0.10)",
					"rgba(58,75,74,0.9)",
				))
					push(f);
				// Gate plane
				const gy = (g / 100) * H;
				push({
					pts: [
						[cx - 0.62, gy, -0.62],
						[cx + 0.62, gy, -0.62],
						[cx + 0.62, gy, 0.62],
						[cx - 0.62, gy, 0.62],
					],
					fill: over ? "rgba(238,91,69,0.10)" : "rgba(226,163,60,0.06)",
					stroke: over ? "rgba(238,91,69,0.8)" : "rgba(226,163,60,0.55)",
				});
				// Tenant slabs and their usage cores
				let y = 0;
				t.slabs.forEach((s, si) => {
					const h = (s.amount / (t.capacity || 1)) * H;
					if (h <= 0) return;
					const c = tenantColors[si % tenantColors.length];
					for (const f of box(
						cx,
						0,
						W * 0.9,
						y + 0.015,
						y + h - 0.015,
						hex(c, 0.13),
						hex(c, 0.75),
					))
						push(f);
					if (s.used !== null && s.amount > 0) {
						const r = Math.min(1, s.used / s.amount);
						const ch = Math.max(0.025, r * (h - 0.06));
						for (const f of box(
							cx,
							0,
							W * 0.5,
							y + 0.03,
							y + 0.03 + ch,
							hex(c, 0.5),
							hex(c, 0.95),
						))
							push(f);
					}
					y += h;
				});
			});

			faces.sort((a, b) => b.depth - a.depth);
			for (const f of faces) {
				const p = f.pts.map(project);
				ctx.beginPath();
				ctx.moveTo(p[0][0], p[0][1]);
				for (let i = 1; i < p.length; i++) ctx.lineTo(p[i][0], p[i][1]);
				ctx.closePath();
				ctx.fillStyle = f.fill;
				ctx.fill();
				ctx.strokeStyle = f.stroke;
				ctx.lineWidth = 0.8;
				ctx.stroke();
			}

			// Work motes: spawn inside slabs, more for busier tenants.
			ts.forEach((t, ti) => {
				let y = 0;
				for (const s of t.slabs) {
					const h = (s.amount / (t.capacity || 1)) * H;
					const busy =
						s.used !== null && s.amount > 0
							? Math.min(1, s.used / s.amount)
							: 0;
					if (h > 0 && !still && Math.random() < dt * (0.6 + busy * 6)) {
						motes.push({
							tower: ti,
							x: xs[ti] + (Math.random() - 0.5) * W * 0.8,
							z: (Math.random() - 0.5) * W * 0.8,
							y: y + Math.random() * h * 0.3,
							top: y + h,
							v: 0.25 + Math.random() * 0.35,
						});
					}
					y += h;
				}
			});
			for (const m of motes) m.y += m.v * dt;
			motes = motes
				.filter((m) => m.y < m.top && m.tower < ts.length)
				.slice(-160);
			for (const m of motes) {
				const [px, py] = project([m.x, m.y, m.z]);
				const a = Math.max(0, 1 - (m.y / m.top) ** 6);
				ctx.fillStyle = `rgba(155,232,223,${0.75 * a})`;
				ctx.fillRect(px - 1, py - 1, 2, 2);
			}

			// Labels
			ctx.textAlign = "center";
			ts.forEach((t, ti) => {
				const committed = t.slabs.reduce((s, x) => s + x.amount, 0);
				const pct = t.capacity ? (committed / t.capacity) * 100 : 0;
				const [bx, by] = project([xs[ti], 0, 0.95]);
				ctx.font = "600 10px Archivo, sans-serif";
				ctx.fillStyle = "#A6B1AE";
				ctx.fillText(t.label.toUpperCase(), bx, by + 18);
				const [tx, ty] = project([xs[ti], H, 0]);
				ctx.font = "500 11px 'IBM Plex Mono', monospace";
				ctx.fillStyle = pct > g ? "#EE5B45" : "#F2F3EE";
				ctx.fillText(`${Math.round(pct)}%`, tx, ty - 16);
			});
			const [gx, gy] = project([xs[n - 1] + 0.62, (g / 100) * H, -0.62]);
			ctx.textAlign = "left";
			ctx.font = "500 9px 'IBM Plex Mono', monospace";
			ctx.fillStyle = "rgba(226,163,60,0.85)";
			ctx.fillText(`GATE ${g}%`, gx + 8, gy + 3);
		};

		const tick = (now: number) => {
			const dt = Math.min(0.05, (now - last) / 1000);
			last = now;
			if (!drag && !still) {
				idle += dt;
				if (idle > 1.5) {
					sway += dt * 0.22;
					yaw = base + 0.28 * Math.sin(sway);
				}
			}
			draw(dt);
			frame = requestAnimationFrame(tick);
		};
		frame = requestAnimationFrame(tick);

		const down = (e: PointerEvent) => {
			drag = { x: e.clientX, y: e.clientY, yaw, pitch };
			el.setPointerCapture(e.pointerId);
		};
		const move = (e: PointerEvent) => {
			if (!drag) return;
			yaw = drag.yaw + (e.clientX - drag.x) * 0.008;
			pitch = Math.max(
				0.12,
				Math.min(1.2, drag.pitch + (e.clientY - drag.y) * 0.005),
			);
		};
		const up = () => {
			drag = null;
			idle = 0;
			base = yaw;
			sway = 0;
		};
		const vis = () => {
			cancelAnimationFrame(frame);
			if (!document.hidden) {
				last = performance.now();
				frame = requestAnimationFrame(tick);
			}
		};
		el.addEventListener("pointerdown", down);
		el.addEventListener("pointermove", move);
		el.addEventListener("pointerup", up);
		el.addEventListener("pointercancel", up);
		document.addEventListener("visibilitychange", vis);
		return () => {
			cancelAnimationFrame(frame);
			ro.disconnect();
			el.removeEventListener("pointerdown", down);
			el.removeEventListener("pointermove", move);
			el.removeEventListener("pointerup", up);
			el.removeEventListener("pointercancel", up);
			document.removeEventListener("visibilitychange", vis);
		};
	}, []);

	return (
		<canvas
			ref={canvas}
			className="block h-[380px] w-full cursor-grab touch-none active:cursor-grabbing"
			aria-label="3D view of each resource: tenant quotas stacked against cluster capacity, with live usage inside"
			role="img"
		/>
	);
}
