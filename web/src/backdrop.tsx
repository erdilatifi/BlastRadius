import { useEffect, useRef } from "react";

type P = { x: number; y: number; vx: number; vy: number; r: number };

const LINK = 130;

/** Drifting particles that link up when close and lean toward the pointer. */
function Particles() {
	const ref = useRef<HTMLCanvasElement>(null);

	useEffect(() => {
		const canvas = ref.current;
		const ctx = canvas?.getContext("2d");
		if (!canvas || !ctx) return;
		const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
		const pointer = { x: -9999, y: -9999 };
		let w = 0;
		let h = 0;
		let points: P[] = [];
		let frame = 0;

		const resize = () => {
			const dpr = Math.min(devicePixelRatio || 1, 2);
			w = innerWidth;
			h = innerHeight;
			canvas.width = w * dpr;
			canvas.height = h * dpr;
			ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
			const count = Math.min(110, Math.round((w * h) / 16000));
			points = Array.from({ length: count }, () => ({
				x: Math.random() * w,
				y: Math.random() * h,
				vx: (Math.random() - 0.5) * 0.25,
				vy: (Math.random() - 0.5) * 0.25,
				r: Math.random() * 1.2 + 0.4,
			}));
		};

		const draw = () => {
			ctx.clearRect(0, 0, w, h);
			for (const p of points) {
				if (!still) {
					const dx = pointer.x - p.x;
					const dy = pointer.y - p.y;
					const d = Math.hypot(dx, dy);
					if (d < 180 && d > 1) {
						p.vx += (dx / d) * 0.006;
						p.vy += (dy / d) * 0.006;
					}
					p.vx *= 0.995;
					p.vy *= 0.995;
					p.x += p.vx;
					p.y += p.vy;
					if (p.x < -10) p.x = w + 10;
					if (p.x > w + 10) p.x = -10;
					if (p.y < -10) p.y = h + 10;
					if (p.y > h + 10) p.y = -10;
				}
			}
			for (let i = 0; i < points.length; i++) {
				const a = points[i];
				for (let j = i + 1; j < points.length; j++) {
					const b = points[j];
					const d = Math.hypot(a.x - b.x, a.y - b.y);
					if (d < LINK) {
						ctx.strokeStyle = `rgba(0,175,174,${(1 - d / LINK) * 0.16})`;
						ctx.lineWidth = 0.6;
						ctx.beginPath();
						ctx.moveTo(a.x, a.y);
						ctx.lineTo(b.x, b.y);
						ctx.stroke();
					}
				}
				const near = Math.hypot(pointer.x - a.x, pointer.y - a.y) < 160;
				ctx.fillStyle = near ? "rgba(0,175,174,0.95)" : "rgba(242,243,238,0.4)";
				ctx.beginPath();
				ctx.arc(a.x, a.y, near ? a.r + 0.6 : a.r, 0, Math.PI * 2);
				ctx.fill();
			}
			if (!still && !document.hidden) frame = requestAnimationFrame(draw);
		};

		const move = (e: PointerEvent) => {
			pointer.x = e.clientX;
			pointer.y = e.clientY;
		};
		const visible = () => {
			cancelAnimationFrame(frame);
			if (!document.hidden) frame = requestAnimationFrame(draw);
		};

		resize();
		draw();
		addEventListener("resize", resize);
		addEventListener("pointermove", move);
		document.addEventListener("visibilitychange", visible);
		return () => {
			cancelAnimationFrame(frame);
			removeEventListener("resize", resize);
			removeEventListener("pointermove", move);
			document.removeEventListener("visibilitychange", visible);
		};
	}, []);

	return <canvas ref={ref} className="absolute inset-0 size-full" />;
}

/** Slow-turning wireframe geometry in the corners of the stage. */
function Geometry() {
	return (
		<svg
			className="absolute inset-0 size-full"
			preserveAspectRatio="xMidYMid slice"
			viewBox="0 0 1600 1000"
			aria-hidden="true"
		>
			<defs>
				<radialGradient id="halo" cx="50%" cy="50%" r="50%">
					<stop offset="0%" stopColor="#00afae" stopOpacity="0.16" />
					<stop offset="100%" stopColor="#00afae" stopOpacity="0" />
				</radialGradient>
			</defs>
			<circle cx="1380" cy="140" r="420" fill="url(#halo)" />
			<circle cx="160" cy="980" r="360" fill="url(#halo)" opacity="0.6" />
			<g fill="none" stroke="#00afae" strokeOpacity="0.16">
				<g className="spin-slow">
					<circle cx="1380" cy="140" r="300" strokeDasharray="2 10" />
					<polygon points="1380,-140 1622,0 1622,280 1380,420 1138,280 1138,0" />
				</g>
				<g className="spin-rev">
					<circle cx="1380" cy="140" r="210" />
					<polygon points="1380,-50 1544,235 1216,235" />
					<line x1="1080" y1="140" x2="1680" y2="140" strokeDasharray="4 8" />
				</g>
				<g className="spin-rev">
					<circle cx="160" cy="980" r="250" strokeDasharray="1 7" />
					<rect x="35" y="855" width="250" height="250" />
				</g>
			</g>
			<g stroke="#f2f3ee" strokeOpacity="0.05">
				<line x1="0" y1="620" x2="1600" y2="300" />
				<line x1="400" y1="0" x2="900" y2="1000" />
			</g>
		</svg>
	);
}

export function Backdrop() {
	return (
		<div className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
			<div className="blueprint absolute inset-0" />
			<Geometry />
			<Particles />
			<div className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,transparent_40%,var(--color-bg)_100%)]" />
		</div>
	);
}
