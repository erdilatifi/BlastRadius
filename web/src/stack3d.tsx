import { useEffect, useRef } from "react";

// A small hand-rolled 3D renderer: no WebGL, no library. Each resource is a
// glass tower as tall as the cluster's capacity; each tenant's quota is a slab
// stacked inside it, and a solid core inside the slab rises with what the
// tenant is actually using. Faces are lit by one fixed light, drawn back to
// front, and the camera frames the whole scene at any canvas size.

export type Slab = { tenant: string; amount: number; used: number | null };

export type Tower = {
	label: string;
	unit: string;
	capacity: number;
	slabs: Slab[];
};

type V3 = [number, number, number];
type RGB = [number, number, number];
type Quad = { pts: V3[]; n: V3 };

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
const FOCAL = 14; // perspective strength: smaller is stronger
const YAW = -0.42;
const PITCH = 0.5;

const rgb = (c: string): RGB => {
	const n = Number.parseInt(c.slice(1), 16);
	return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};
const css = ([r, g, b]: RGB, a: number) =>
	`rgba(${r | 0},${g | 0},${b | 0},${a.toFixed(3)})`;
/** Darken below 1, lighten toward white above 1. */
const shade = (c: RGB, k: number): RGB =>
	c.map((v) => (k < 1 ? v * k : v + (255 - v) * (k - 1))) as RGB;
const norm = ([x, y, z]: V3): V3 => {
	const l = Math.hypot(x, y, z);
	return [x / l, y / l, z / l];
};

const LIGHT = norm([-0.55, 0.95, 0.65]);
const GLASS = rgb("#3a4b4a");
const GATE = rgb("#e2a33c");
const CRIT = rgb("#ee5b45");

function box(cx: number, cz: number, w: number, y0: number, y1: number) {
	const h = w / 2;
	const [x0, x1, z0, z1] = [cx - h, cx + h, cz - h, cz + h];
	const q = (n: V3, ...pts: V3[]): Quad => ({ pts, n });
	return [
		q([0, 0, -1], [x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]),
		q([0, 0, 1], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]),
		q([-1, 0, 0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]),
		q([1, 0, 0], [x1, y0, z0], [x1, y0, z1], [x1, y1, z1], [x1, y1, z0]),
		q([0, 1, 0], [x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]),
		q([0, -1, 0], [x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]),
	];
}

/** Convex hull (monotone chain) of screen points, for hit testing. */
function hull(points: [number, number][]) {
	const p = [...points].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
	const cross = (o: number[], a: number[], b: number[]) =>
		(a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
	const half = (pts: [number, number][]) => {
		const out: [number, number][] = [];
		for (const q of pts) {
			while (
				out.length >= 2 &&
				cross(out[out.length - 2], out[out.length - 1], q) <= 0
			)
				out.pop();
			out.push(q);
		}
		out.pop();
		return out;
	};
	return [...half(p), ...half([...p].reverse())];
}

const inside = (poly: [number, number][], x: number, y: number) => {
	let hit = false;
	for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
		const [xi, yi] = poly[i];
		const [xj, yj] = poly[j];
		if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi)
			hit = !hit;
	}
	return hit;
};

type Mote = {
	tower: number;
	slab: number;
	x: number;
	z: number;
	y: number;
	v: number;
};

type Hit = {
	tenant: string;
	tower: number;
	slab: number;
	poly: [number, number][];
	depth: number;
};

const amountText = (v: number, unit: string) =>
	`${Number(v.toFixed(v < 10 ? 1 : 0))}${unit}`;

export function Stack3D({
	towers,
	gate,
	focus = null,
	onFocus,
}: {
	towers: Tower[];
	gate: number;
	/** Tenant to highlight, from outside the canvas (the legend). */
	focus?: string | null;
	/** Called with the tenant under the pointer, or null. */
	onFocus?: (tenant: string | null) => void;
}) {
	const canvas = useRef<HTMLCanvasElement>(null);
	const props = useRef({ towers, gate, focus, onFocus });
	props.current = { towers, gate, focus, onFocus };

	useEffect(() => {
		const el = canvas.current;
		const ctx = el?.getContext("2d");
		if (!el || !ctx) return;
		const still = matchMedia("(prefers-reduced-motion: reduce)").matches;

		// Camera: drag to orbit, a little inertia on release, then a slow sway
		// around wherever the viewer left it.
		let base = YAW;
		let yaw = YAW;
		let pitch = PITCH;
		let spin = 0;
		let sway = 0;
		let idle = 0;
		let drag: {
			x: number;
			y: number;
			yaw: number;
			pitch: number;
			t: number;
			touch: boolean;
		} | null = null;

		let size = { w: 0, h: 0 };
		const fit = { s: 0, x: 0, y: 0 };
		let frame = 0;
		let last = performance.now();
		let onScreen = true;
		let motes: Mote[] = [];
		let hits: Hit[] = [];
		let pointer: { x: number; y: number } | null = null;
		let hovered: string | null = null;
		// Shown values ease toward the data, so a refresh glides instead of jumping.
		const shown = new Map<string, { amount: number; used: number }>();

		const resize = () => {
			const r = el.getBoundingClientRect();
			const dpr = Math.min(devicePixelRatio || 1, 2);
			size = { w: r.width, h: r.height };
			el.width = Math.round(r.width * dpr);
			el.height = Math.round(r.height * dpr);
			ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
			fit.s = 0;
		};
		resize();
		const ro = new ResizeObserver(resize);
		ro.observe(el);
		const io = new IntersectionObserver(([e]) => {
			onScreen = e.isIntersecting;
			wake();
		});
		io.observe(el);

		// Camera space: x right, y up, depth grows away from the viewer.
		const view = ([x, y, z]: V3): V3 => {
			const cy = Math.cos(yaw);
			const sy = Math.sin(yaw);
			const x1 = x * cy - z * sy;
			const z1 = x * sy + z * cy;
			const cp = Math.cos(pitch);
			const sp = Math.sin(pitch);
			return [x1, y * cp - z1 * sp, -(y * sp + z1 * cp)];
		};
		const raw = (p: V3): [number, number, number] => {
			const [x, y, d] = view([p[0], p[1] - H / 2, p[2]]);
			const k = FOCAL / (FOCAL + d);
			return [x * k, -y * k, d];
		};
		const project = (p: V3): [number, number, number] => {
			const [x, y, d] = raw(p);
			return [fit.x + x * fit.s, fit.y + y * fit.s, d];
		};
		const facing = (n: V3) => view(n)[2] < 0;
		const lit = (n: V3) =>
			0.42 +
			0.78 * Math.max(0, n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2]);

		const path = (pts: V3[]) => {
			ctx.beginPath();
			pts.forEach((p, i) => {
				const [x, y] = project(p);
				if (i) ctx.lineTo(x, y);
				else ctx.moveTo(x, y);
			});
			ctx.closePath();
		};
		const face = (q: Quad, fill: string | null, stroke: string, lw = 0.8) => {
			path(q.pts);
			if (fill) {
				ctx.fillStyle = fill;
				ctx.fill();
			}
			ctx.strokeStyle = stroke;
			ctx.lineWidth = lw;
			ctx.stroke();
		};

		const draw = (dt: number) => {
			const { towers: ts, gate: g, focus: outside } = props.current;
			const focus = hovered ?? outside;
			const n = ts.length;
			const xs = ts.map((_, i) => (i - (n - 1) / 2) * GAP);
			const narrow = size.w < 480;

			// Frame the towers plus room for their labels.
			const pad = { x: narrow ? 18 : 40, top: 46, bottom: 52 };
			let [minX, minY, maxX, maxY] = [
				Number.POSITIVE_INFINITY,
				Number.POSITIVE_INFINITY,
				Number.NEGATIVE_INFINITY,
				Number.NEGATIVE_INFINITY,
			];
			for (const cx of xs)
				for (const q of box(cx, 0, W * 1.3, 0, H))
					for (const p of q.pts) {
						const [x, y] = raw(p);
						minX = Math.min(minX, x);
						maxX = Math.max(maxX, x);
						minY = Math.min(minY, y);
						maxY = Math.max(maxY, y);
					}
			if (n && size.w > 0) {
				const s = Math.min(
					(size.w - 2 * pad.x) / (maxX - minX),
					(size.h - pad.top - pad.bottom) / (maxY - minY),
				);
				const x = size.w / 2 - ((minX + maxX) / 2) * s;
				const y =
					pad.top +
					(size.h - pad.top - pad.bottom) / 2 -
					((minY + maxY) / 2) * s;
				const k = fit.s === 0 || still ? 1 : Math.min(1, dt * 5);
				fit.s += (s - fit.s) * k;
				fit.x += (x - fit.x) * k;
				fit.y += (y - fit.y) * k;
			}

			ctx.clearRect(0, 0, size.w, size.h);

			// Floor grid that fades out with distance from the centre.
			const span = Math.max(3, (n * GAP) / 2 + 1.2);
			const depthSpan = 2.2;
			ctx.lineWidth = 1;
			const seg = (a: V3, b: V3, major: boolean) => {
				const mid = Math.hypot(
					(a[0] + b[0]) / 2 / span,
					(a[2] + b[2]) / 2 / depthSpan,
				);
				const alpha = (major ? 0.5 : 0.28) * Math.max(0, 1 - mid) ** 1.4;
				if (alpha < 0.01) return;
				const p = project(a);
				const q = project(b);
				ctx.strokeStyle = `rgba(58,75,74,${alpha.toFixed(3)})`;
				ctx.beginPath();
				ctx.moveTo(p[0], p[1]);
				ctx.lineTo(q[0], q[1]);
				ctx.stroke();
			};
			for (let i = -span; i <= span + 1e-6; i += 0.5) {
				const major = Math.abs(i % 1) < 1e-6;
				for (let z = -depthSpan; z < depthSpan - 1e-6; z += 0.5)
					seg([i, 0, z], [i, 0, z + 0.5], major);
			}
			for (let z = -depthSpan; z <= depthSpan + 1e-6; z += 0.5) {
				const major = Math.abs(z % 1) < 1e-6;
				for (let i = -span; i < span - 1e-6; i += 0.5)
					seg([i, 0, z], [i + 0.5, 0, z], major);
			}

			// Ease shown values toward the data.
			const ease = still ? 1 : 1 - Math.exp(-dt * 4);
			ts.forEach((t, ti) => {
				t.slabs.forEach((s, si) => {
					const key = `${ti}:${si}`;
					const cur = shown.get(key) ?? { amount: s.amount, used: s.used ?? 0 };
					cur.amount += (s.amount - cur.amount) * ease;
					cur.used += ((s.used ?? 0) - cur.used) * ease;
					shown.set(key, cur);
				});
			});

			// Towers far to near.
			const order = ts
				.map((_, i) => i)
				.sort((a, b) => raw([xs[b], H / 2, 0])[2] - raw([xs[a], H / 2, 0])[2]);
			const nextHits: Hit[] = [];

			for (const ti of order) {
				const t = ts[ti];
				const cx = xs[ti];
				const cap = t.capacity || 1;
				const committed = t.slabs.reduce((s, x) => s + x.amount, 0);
				const over = (committed / cap) * 100 > g;

				// Soft pool of light under the tower.
				const [fx, fy] = project([cx, 0, 0]);
				const [ex] = project([cx + W * 1.1, 0, 0]);
				const r = Math.max(8, Math.abs(ex - fx));
				const pool = ctx.createRadialGradient(fx, fy, 0, fx, fy, r);
				pool.addColorStop(
					0,
					over ? "rgba(238,91,69,0.22)" : "rgba(0,175,174,0.2)",
				);
				pool.addColorStop(1, "rgba(0,175,174,0)");
				ctx.fillStyle = pool;
				ctx.beginPath();
				for (let a = 0; a <= 24; a++) {
					const th = (a / 24) * Math.PI * 2;
					const [px, py] = project([
						cx + Math.cos(th) * W * 1.1,
						0,
						Math.sin(th) * W * 1.1,
					]);
					if (a) ctx.lineTo(px, py);
					else ctx.moveTo(px, py);
				}
				ctx.fill();

				const shell = box(cx, 0, W, 0, H);
				const slabs: {
					quads: Quad[];
					core: Quad[] | null;
					c: RGB;
					si: number;
					tenant: string;
				}[] = [];
				let y = 0;
				t.slabs.forEach((s, si) => {
					const v = shown.get(`${ti}:${si}`) ?? { amount: 0, used: 0 };
					const h = (v.amount / cap) * H;
					if (h <= 0.002) return;
					const c = rgb(tenantColors[si % tenantColors.length]);
					const quads = box(cx, 0, W * 0.88, y + 0.012, y + h - 0.012);
					let core: Quad[] | null = null;
					if (s.used !== null && v.amount > 0) {
						const ratio = Math.min(1, v.used / v.amount);
						const ch = Math.max(0.02, ratio * (h - 0.05));
						core = box(cx, 0, W * 0.52, y + 0.025, y + 0.025 + ch);
					}
					slabs.push({ quads, core, c, si, tenant: s.tenant });
					const pts = quads.flatMap((q) => q.pts).map((p) => project(p));
					nextHits.push({
						tenant: s.tenant,
						tower: ti,
						slab: si,
						poly: hull(pts.map(([px, py]) => [px, py])),
						depth: raw([cx, y + h / 2, 0])[2],
					});
					y += h;
				});
				const dim = (tenant: string) => (focus && focus !== tenant ? 0.3 : 1);

				// 1. Back of the glass shell.
				for (const q of shell)
					if (!facing(q.n)) face(q, css(GLASS, 0.05), css(GLASS, 0.45), 0.6);
				// 2. Back faces of every slab, faint.
				for (const s of slabs)
					for (const q of s.quads)
						if (!facing(q.n))
							face(
								q,
								css(shade(s.c, lit(q.n) * 0.6), 0.06 * dim(s.tenant)),
								css(s.c, 0.22 * dim(s.tenant)),
								0.6,
							);
				// 3. Solid usage cores, front faces only, lit.
				for (const s of slabs) {
					if (!s.core) continue;
					const a = dim(s.tenant);
					for (const q of s.core)
						if (facing(q.n)) {
							const k = lit(q.n);
							face(
								q,
								css(shade(s.c, k * 0.85), 0.92 * a),
								css(shade(s.c, Math.max(1.05, k)), 0.9 * a),
								0.7,
							);
						}
					// The fill line: a bright rim on the core's top face.
					const top = s.core[4];
					path(top.pts);
					ctx.strokeStyle = css(shade(s.c, 1.45), 0.95 * a);
					ctx.lineWidth = 1.2;
					ctx.stroke();
				}
				// 4. Front faces of the slabs: tinted glass, lit, sorted by depth.
				const fronts = slabs
					.flatMap((s) =>
						s.quads.filter((q) => facing(q.n)).map((q) => ({ q, s })),
					)
					.map((f) => ({
						...f,
						d: f.q.pts.reduce((sum, p) => sum + raw(p)[2], 0) / 4,
					}))
					.sort((a, b) => b.d - a.d);
				for (const { q, s } of fronts) {
					const a = dim(s.tenant);
					const k = lit(q.n);
					const hot = focus === s.tenant;
					face(
						q,
						css(
							shade(s.c, k),
							(q.n[1] > 0 ? 0.2 : 0.12) * a + (hot ? 0.08 : 0),
						),
						css(shade(s.c, hot ? 1.35 : 1), (hot ? 1 : 0.7) * a),
						hot ? 1.2 : 0.8,
					);
				}
				// 5. The gate plane, dashed, red when the tower is over it.
				const gy = (g / 100) * H;
				const gc = over ? CRIT : GATE;
				const gq: Quad = {
					n: [0, 1, 0],
					pts: [
						[cx - 0.64, gy, -0.64],
						[cx + 0.64, gy, -0.64],
						[cx + 0.64, gy, 0.64],
						[cx - 0.64, gy, 0.64],
					],
				};
				path(gq.pts);
				ctx.fillStyle = css(gc, over ? 0.12 : 0.07);
				ctx.fill();
				ctx.setLineDash([4, 3]);
				ctx.strokeStyle = css(gc, over ? 0.9 : 0.6);
				ctx.lineWidth = 1;
				ctx.stroke();
				ctx.setLineDash([]);
				// 6. Front of the glass shell, with a highlight on the lit edges.
				for (const q of shell)
					if (facing(q.n)) {
						const k = lit(q.n);
						face(
							q,
							css(shade(GLASS, k), 0.05),
							css(shade(GLASS, 0.9 + k * 0.5), 0.85),
							0.9,
						);
					}
			}
			hits = nextHits;

			// Work motes: rise through each slab, faster and denser when busy.
			if (!still) {
				ts.forEach((t, ti) => {
					t.slabs.forEach((s, si) => {
						const busy =
							s.used !== null && s.amount > 0
								? Math.min(1, s.used / s.amount)
								: 0;
						if (s.amount > 0 && Math.random() < dt * (0.5 + busy * 5)) {
							motes.push({
								tower: ti,
								slab: si,
								x: xs[ti] + (Math.random() - 0.5) * W * 0.78,
								z: (Math.random() - 0.5) * W * 0.78,
								y: Math.random() * 0.3,
								v: 0.18 + busy * 0.4 + Math.random() * 0.2,
							});
						}
					});
				});
				ctx.globalCompositeOperation = "lighter";
				const alive: Mote[] = [];
				for (const m of motes) {
					const t = ts[m.tower];
					if (!t?.slabs[m.slab]) continue;
					const cap = t.capacity || 1;
					let y0 = 0;
					for (let i = 0; i < m.slab; i++) y0 += (t.slabs[i].amount / cap) * H;
					const h = (t.slabs[m.slab].amount / cap) * H;
					m.y += m.v * dt;
					if (m.y >= h) continue;
					alive.push(m);
					const [px, py] = project([m.x, y0 + m.y, m.z]);
					const fade = Math.sin((m.y / h) * Math.PI);
					const c = rgb(tenantColors[m.slab % tenantColors.length]);
					const a =
						0.8 * fade * (focus && focus !== t.slabs[m.slab].tenant ? 0.25 : 1);
					ctx.fillStyle = css(shade(c, 1.4), a);
					ctx.beginPath();
					ctx.arc(px, py, 1.3, 0, Math.PI * 2);
					ctx.fill();
				}
				ctx.globalCompositeOperation = "source-over";
				motes = alive.slice(-220);
			}

			// Labels.
			ctx.textAlign = "center";
			ts.forEach((t, ti) => {
				const cx = xs[ti];
				const committed = t.slabs.reduce((s, x) => s + x.amount, 0);
				const pct = t.capacity ? (committed / t.capacity) * 100 : 0;
				const front = project([cx, 0, W * 0.5 + 0.35]);
				ctx.font = `600 ${narrow ? 10 : 11}px Archivo, sans-serif`;
				ctx.fillStyle = "#A6B1AE";
				ctx.fillText(t.label.toUpperCase(), front[0], front[1] + 16);
				ctx.font = `400 ${narrow ? 9 : 10}px 'IBM Plex Mono', monospace`;
				ctx.fillStyle = "#6B7A77";
				ctx.fillText(
					`${amountText(committed, t.unit)} / ${amountText(t.capacity, t.unit)}`,
					front[0],
					front[1] + 30,
				);
				const top = project([cx, H, 0]);
				ctx.font = `500 ${narrow ? 12 : 14}px 'IBM Plex Mono', monospace`;
				ctx.fillStyle = pct > g ? "#EE5B45" : "#F2F3EE";
				ctx.fillText(`${Math.round(pct)}%`, top[0], top[1] - 14);
			});
			if (n) {
				// Put the gate label beside whichever gate corner is furthest right.
				const gy = (g / 100) * H;
				let best: [number, number, number] = [Number.NEGATIVE_INFINITY, 0, 0];
				for (const cx of xs)
					for (const [x, z] of [
						[0.64, 0.64],
						[0.64, -0.64],
					]) {
						const p = project([cx + x, gy, z]);
						if (p[0] > best[0]) best = p;
					}
				ctx.textAlign = "left";
				ctx.font = "500 9px 'IBM Plex Mono', monospace";
				ctx.fillStyle = "rgba(226,163,60,0.9)";
				const label = `GATE ${g}%`;
				const x = Math.min(
					best[0] + 6,
					size.w - ctx.measureText(label).width - 4,
				);
				ctx.fillText(label, x, best[1] + 3);
			}

			// Read-out for the slab under the pointer.
			if (hovered && pointer) {
				const hit = hits.find(
					(h) =>
						h.tenant === hovered &&
						inside(h.poly, pointer?.x ?? 0, pointer?.y ?? 0),
				);
				const t = hit && ts[hit.tower];
				const s = hit && t?.slabs[hit.slab];
				if (t && s) {
					const lines = [
						`${s.tenant} · ${t.label}`,
						`quota ${amountText(s.amount, t.unit)} (${Math.round((s.amount / (t.capacity || 1)) * 100)}% of cluster)`,
						s.used === null
							? "usage not measured"
							: `using ${amountText(s.used, t.unit)} (${Math.round((s.used / (s.amount || 1)) * 100)}% of quota)`,
					];
					ctx.font = "500 11px 'IBM Plex Mono', monospace";
					const w =
						Math.max(...lines.map((l) => ctx.measureText(l).width)) + 20;
					const h = lines.length * 16 + 12;
					const x = Math.min(Math.max(4, pointer.x + 14), size.w - w - 4);
					const y = Math.min(Math.max(4, pointer.y - h - 10), size.h - h - 4);
					ctx.fillStyle = "rgba(7,11,11,0.92)";
					ctx.strokeStyle = "rgba(0,175,174,0.55)";
					ctx.lineWidth = 1;
					ctx.fillRect(x, y, w, h);
					ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
					ctx.textAlign = "left";
					lines.forEach((l, i) => {
						ctx.fillStyle = i ? "#A6B1AE" : "#F2F3EE";
						ctx.fillText(l, x + 10, y + 20 + i * 16);
					});
				}
			}
		};

		const pick = () => {
			if (!pointer || drag) return null;
			let best: Hit | null = null;
			for (const h of hits)
				if (
					inside(h.poly, pointer.x, pointer.y) &&
					(!best || h.depth < best.depth)
				)
					best = h;
			return best?.tenant ?? null;
		};
		const setHover = (tenant: string | null) => {
			if (tenant === hovered) return;
			hovered = tenant;
			props.current.onFocus?.(tenant);
		};

		const tick = (now: number) => {
			frame = 0;
			const dt = Math.min(0.05, (now - last) / 1000);
			last = now;
			if (!drag && !still) {
				if (Math.abs(spin) > 0.002) {
					yaw += spin * dt;
					spin *= Math.exp(-dt * 3.5);
					base = yaw;
					sway = 0;
				} else {
					idle += dt;
					if (idle > 2.5 && !hovered) {
						sway += dt * 0.2;
						yaw = base + 0.26 * Math.sin(sway);
					}
				}
			}
			draw(dt);
			wake();
		};
		function wake() {
			if (frame || !onScreen || document.hidden) return;
			last = performance.now();
			frame = requestAnimationFrame(tick);
		}
		wake();

		const local = (e: PointerEvent) => {
			const r = el.getBoundingClientRect();
			return { x: e.clientX - r.left, y: e.clientY - r.top };
		};
		const down = (e: PointerEvent) => {
			drag = {
				x: e.clientX,
				y: e.clientY,
				yaw,
				pitch,
				t: performance.now(),
				touch: e.pointerType === "touch",
			};
			spin = 0;
			setHover(null);
			el.setPointerCapture(e.pointerId);
		};
		const move = (e: PointerEvent) => {
			pointer = local(e);
			if (!drag) {
				setHover(pick());
				return;
			}
			const before = yaw;
			yaw = drag.yaw + (e.clientX - drag.x) * 0.008;
			// Touch only orbits sideways, so a vertical swipe still scrolls the page.
			if (!drag.touch)
				pitch = Math.max(
					0.15,
					Math.min(1.15, drag.pitch + (e.clientY - drag.y) * 0.005),
				);
			const now = performance.now();
			spin = ((yaw - before) / Math.max(1, now - drag.t)) * 1000;
			drag.t = now;
		};
		const up = () => {
			if (!drag) return;
			drag = null;
			idle = 0;
			base = yaw;
			sway = 0;
			if (still) spin = 0;
		};
		const leave = () => {
			pointer = null;
			setHover(null);
		};
		const reset = () => {
			yaw = base = YAW;
			pitch = PITCH;
			spin = sway = idle = 0;
		};
		const vis = () => {
			if (document.hidden) {
				cancelAnimationFrame(frame);
				frame = 0;
			} else wake();
		};
		el.addEventListener("pointerdown", down);
		el.addEventListener("pointermove", move);
		el.addEventListener("pointerup", up);
		el.addEventListener("pointercancel", up);
		el.addEventListener("pointerleave", leave);
		el.addEventListener("dblclick", reset);
		document.addEventListener("visibilitychange", vis);
		return () => {
			cancelAnimationFrame(frame);
			ro.disconnect();
			io.disconnect();
			el.removeEventListener("pointerdown", down);
			el.removeEventListener("pointermove", move);
			el.removeEventListener("pointerup", up);
			el.removeEventListener("pointercancel", up);
			el.removeEventListener("pointerleave", leave);
			el.removeEventListener("dblclick", reset);
			document.removeEventListener("visibilitychange", vis);
		};
	}, []);

	return (
		<canvas
			ref={canvas}
			className="block h-[300px] w-full cursor-grab touch-pan-y active:cursor-grabbing sm:h-[380px] lg:h-[440px] xl:h-[480px]"
			aria-label="3D view of each resource: tenant quotas stacked against cluster capacity, with live usage inside"
			role="img"
		/>
	);
}
