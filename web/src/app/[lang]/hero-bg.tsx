"use client";

import { useRef, useMemo, useState, useEffect } from "react";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import * as THREE from "three";

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   "The Living Workshop" — Agent Hand Hero Background  v2

   18 session entities spread wide to L/R margins, 10 floating terminal
   window wireframes, 12 relationship streams, ember particles,
   priority pulse, remote portal, canvas grid, ambient star field.
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

// ─── Types ───

type Status = "fire" | "running" | "waiting" | "starting" | "idle";
type Tool = "claude" | "gemini" | "shell";
type Geo = "ico" | "dodeca" | "octa";
type RelType = "parent" | "dependency" | "peer" | "collab";

interface Entity {
  id: number;
  pos: [number, number, number];
  status: Status;
  tool: Tool;
  size: number;
  geo: Geo;
}

// ─── Colour Maps ───

const STATUS_HEX: Record<Status, string> = {
  fire: "#eab308", running: "#eab308", waiting: "#3b82f6",
  starting: "#06b6d4", idle: "#64748b",
};
const TOOL_HEX: Record<Tool, string> = {
  claude: "#6366f1", gemini: "#06b6d4", shell: "#22c55e",
};
const REL_HEX: Record<RelType, string> = {
  parent: "#6366f1", dependency: "#eab308", peer: "#06b6d4", collab: "#22c55e",
};

// ─── Scene Data — entities pushed wide to L/R margins, centre left open ───

const ENTITIES: Entity[] = [
  // ── Left cluster — wide spread ──
  { id: 0,  pos: [-8.5,  2.2,  0.5], status: "fire",     tool: "claude",  size: 0.55, geo: "ico"    },
  { id: 1,  pos: [-10.0, 0.0, -0.5], status: "running",  tool: "shell",   size: 0.42, geo: "octa"   },
  { id: 2,  pos: [-7.0, -1.8,  0.3], status: "idle",     tool: "claude",  size: 0.35, geo: "ico"    },
  { id: 3,  pos: [-11.0, 1.5, -2.0], status: "idle",     tool: "shell",   size: 0.30, geo: "ico"    },
  { id: 4,  pos: [-7.5, -3.2, -0.5], status: "starting", tool: "gemini",  size: 0.38, geo: "dodeca" },
  // ── Right cluster — wide spread ──
  { id: 5,  pos: [ 8.5,  2.0, -0.5], status: "waiting",  tool: "claude",  size: 0.50, geo: "dodeca" },
  { id: 6,  pos: [10.5, -0.5,  0.0], status: "running",  tool: "gemini",  size: 0.44, geo: "dodeca" },
  { id: 7,  pos: [ 7.0, -2.0,  0.5], status: "running",  tool: "claude",  size: 0.42, geo: "ico"    },
  { id: 8,  pos: [11.0,  1.0, -1.5], status: "starting", tool: "shell",   size: 0.33, geo: "octa"   },
  { id: 9,  pos: [ 8.0, -3.5, -0.5], status: "idle",     tool: "claude",  size: 0.33, geo: "ico"    },
  // ── Scattered — behind / above / below hero text ──
  { id: 10, pos: [-4.5,  3.8, -3.0], status: "idle",     tool: "claude",  size: 0.28, geo: "ico"    },
  { id: 11, pos: [ 5.0,  3.5, -2.5], status: "running",  tool: "gemini",  size: 0.30, geo: "ico"    },
  { id: 12, pos: [ 0.0, -4.0, -1.0], status: "idle",     tool: "shell",   size: 0.28, geo: "ico"    },
  { id: 13, pos: [-2.5, -4.5,  0.0], status: "starting", tool: "claude",  size: 0.30, geo: "octa"   },
  // ── Extra entities for more density on sides ──
  { id: 14, pos: [-12.0, -1.0, -1.0], status: "running", tool: "gemini",  size: 0.32, geo: "dodeca" },
  { id: 15, pos: [ 12.0,  0.5, -1.0], status: "running", tool: "claude",  size: 0.35, geo: "ico"    },
  { id: 16, pos: [-9.5,  3.5, -1.5], status: "waiting",  tool: "claude",  size: 0.30, geo: "dodeca" },
  { id: 17, pos: [ 9.5, -4.0,  0.0], status: "fire",     tool: "shell",   size: 0.40, geo: "octa"   },
];

const RELS: { from: number; to: number; type: RelType }[] = [
  // Left-side connections
  { from: 0, to: 2, type: "parent"     },   // fire → idle (spawned child)
  { from: 0, to: 1, type: "dependency"  },   // fire → running shell
  { from: 1, to: 4, type: "collab"      },   // shell ↔ starting gemini
  // Right-side connections
  { from: 5, to: 7, type: "dependency"  },   // waiting → running claude
  { from: 6, to: 9, type: "peer"        },   // gemini ↔ idle claude
  { from: 6, to: 8, type: "parent"      },   // gemini spawned starting shell
  // Cross-scene dramatic link
  { from: 0, to: 5, type: "dependency"  },   // fire → waiting (across entire scene)
  // Background peers
  { from: 11, to: 10, type: "peer"      },   // distant gemini ↔ claude
  // New entity connections
  { from: 14, to: 3,  type: "collab"     },   // far-left gemini ↔ idle shell
  { from: 15, to: 8,  type: "dependency" },   // far-right claude → starting shell
  { from: 16, to: 0,  type: "peer"       },   // upper-left waiting → fire
  { from: 17, to: 9,  type: "parent"     },   // lower-right fire spawned idle
];

// ─── Terminal window definitions — floating wireframe terminal/session frames ───

const TERMINALS: { pos: [number, number, number]; w: number; h: number; color: string; id: number }[] = [
  { pos: [-10.0, 1.5,  1.5], w: 1.3, h: 0.85, color: "#6366f1", id: 0 },   // left Claude session
  { pos: [-6.5, -2.5, -0.3], w: 1.0, h: 0.7,  color: "#06b6d4", id: 1 },   // lower-left Gemini
  { pos: [ 10.0, 1.0,  0.5], w: 1.3, h: 0.85, color: "#6366f1", id: 2 },   // right Claude session
  { pos: [ 7.0, -2.8, -0.3], w: 1.0, h: 0.7,  color: "#22c55e", id: 3 },   // lower-right shell
  { pos: [-12.0,-1.0, -1.8], w: 0.9, h: 0.6,  color: "#64748b", id: 4 },   // far-left dim
  { pos: [ 12.0,-0.5, -1.5], w: 0.9, h: 0.6,  color: "#7c3aed", id: 5 },   // far-right violet
  // ── Extra terminal frames — more density ──
  { pos: [-8.0,  3.0, -0.5], w: 0.85, h: 0.6,  color: "#22c55e", id: 6 },   // upper-left shell
  { pos: [ 8.5,  3.2, -0.8], w: 0.85, h: 0.6,  color: "#06b6d4", id: 7 },   // upper-right gemini
  { pos: [-11.0, 3.0, -2.0], w: 0.7, h: 0.5,  color: "#eab308", id: 8 },   // far upper-left
  { pos: [ 11.5,-3.0, -1.0], w: 0.7, h: 0.5,  color: "#6366f1", id: 9 },   // far lower-right
];

// ─── Geometry Helper ───

function EntityGeo({ geo, size }: { geo: Geo; size: number }) {
  switch (geo) {
    case "dodeca": return <dodecahedronGeometry args={[size, 0]} />;
    case "octa":   return <octahedronGeometry args={[size, 0]} />;
    default:       return <icosahedronGeometry args={[size, 1]} />;
  }
}

/* ━━━ 1. Session Entity ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function SessionEntity({ e }: { e: Entity }) {
  const meshRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.Mesh>(null);

  useFrame(({ clock }) => {
    const mesh = meshRef.current;
    if (!mesh) return;
    const t = clock.getElapsedTime();
    const mat = mesh.material as THREE.MeshBasicMaterial;

    mesh.position.x = e.pos[0] + Math.sin(t * 0.3 + e.id * 2.1) * 0.08;
    mesh.position.y = e.pos[1] + Math.sin(t * 0.5 + e.id * 1.3) * 0.15;
    mesh.position.z = e.pos[2];

    switch (e.status) {
      case "fire": {
        mesh.rotation.x += 0.015;
        mesh.rotation.y += 0.02;
        mesh.position.x += Math.sin(t * 15 + e.id) * 0.02;
        mesh.position.y += Math.cos(t * 12 + e.id) * 0.015;
        const flare = Math.pow(Math.max(0, Math.sin(t * 0.4)), 20);
        mat.opacity = 0.8 + Math.sin(t * 4) * 0.15 + flare * 0.3;
        mesh.scale.setScalar(1 + Math.sin(t * 1.5) * 0.08 + flare * 0.2);
        break;
      }
      case "running":
        mesh.rotation.x += 0.008;
        mesh.rotation.y += 0.012;
        mat.opacity = 0.75;
        mesh.scale.setScalar(1 + Math.sin(t * 1.2 + e.id) * 0.05);
        break;
      case "waiting":
        mesh.rotation.y += 0.003;
        mat.opacity = 0.45 + Math.abs(Math.sin(t * 2)) * 0.45;
        break;
      case "starting":
        mesh.rotation.x += 0.01;
        mesh.rotation.z += 0.015;
        mat.opacity = 0.4 + Math.abs(Math.sin(t * 3)) * 0.5;
        mesh.scale.setScalar(0.8 + Math.sin(t * 2) * 0.15);
        break;
      default:
        mesh.rotation.y += 0.002;
        mat.opacity = 0.45;
    }

    const glow = glowRef.current;
    if (glow) {
      glow.position.copy(mesh.position);
      glow.rotation.copy(mesh.rotation);
      glow.scale.setScalar(mesh.scale.x * 1.8);
      const gMat = glow.material as THREE.MeshBasicMaterial;
      gMat.opacity = e.status === "fire"    ? 0.1 + Math.sin(t * 3) * 0.05
                   : e.status === "running" ? 0.08
                   : 0.05;
    }
  });

  return (
    <group>
      <mesh ref={meshRef} position={e.pos}>
        <EntityGeo geo={e.geo} size={e.size} />
        <meshBasicMaterial color={STATUS_HEX[e.status]} wireframe transparent opacity={0.65} />
      </mesh>
      <mesh ref={glowRef} position={e.pos}>
        <EntityGeo geo={e.geo} size={e.size * 1.8} />
        <meshBasicMaterial
          color={TOOL_HEX[e.tool]} wireframe transparent opacity={0.08}
          blending={THREE.AdditiveBlending} depthWrite={false}
        />
      </mesh>
    </group>
  );
}

/* ━━━ 2. Terminal Frame ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   Floating wireframe terminal windows — the "session / window" concept.
   Each frame has a border, title bar with dots, and faint text lines.
   Slightly angled to face the camera, adding depth and variety.
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function TerminalFrame({ pos, w, h, color, id }: {
  pos: [number, number, number]; w: number; h: number; color: string; id: number;
}) {
  const ref = useRef<THREE.LineSegments>(null);

  const geometry = useMemo(() => {
    const hw = w / 2, hh = h / 2;
    const tb = hh - h * 0.18; // title bar y
    const dotY = tb + (hh - tb) / 2;
    const v: number[] = [
      // Outer border
      -hw, -hh, 0,  hw, -hh, 0,
       hw, -hh, 0,  hw,  hh, 0,
       hw,  hh, 0, -hw,  hh, 0,
      -hw,  hh, 0, -hw, -hh, 0,
      // Title bar separator
      -hw, tb, 0,  hw, tb, 0,
      // Title bar "dots" (3 small dashes = close/min/max)
      -hw + 0.07, dotY, 0,  -hw + 0.11, dotY, 0,
      -hw + 0.15, dotY, 0,  -hw + 0.19, dotY, 0,
      -hw + 0.23, dotY, 0,  -hw + 0.27, dotY, 0,
      // Content "text" lines (simulating terminal output)
      -hw + 0.06, tb - h * 0.12, 0,  -hw + w * 0.7,  tb - h * 0.12, 0,
      -hw + 0.06, tb - h * 0.24, 0,  -hw + w * 0.45, tb - h * 0.24, 0,
      -hw + 0.06, tb - h * 0.36, 0,  -hw + w * 0.8,  tb - h * 0.36, 0,
      -hw + 0.06, tb - h * 0.48, 0,  -hw + w * 0.3,  tb - h * 0.48, 0,
      // Cursor line (blinking handled in animation)
      -hw + 0.06, tb - h * 0.60, 0,  -hw + 0.12,     tb - h * 0.60, 0,
    ];
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.Float32BufferAttribute(v, 3));
    return geo;
  }, [w, h]);

  useFrame(({ clock }) => {
    if (!ref.current) return;
    const t = clock.getElapsedTime();
    const mat = ref.current.material as THREE.LineBasicMaterial;
    // Float gently
    ref.current.position.y = pos[1] + Math.sin(t * 0.35 + id * 1.7) * 0.1;
    ref.current.position.x = pos[0] + Math.sin(t * 0.2 + id * 2.3) * 0.05;
    // Face toward centre slightly
    ref.current.rotation.y = (pos[0] > 0 ? -0.25 : 0.25) + Math.sin(t * 0.15 + id) * 0.08;
    ref.current.rotation.x = Math.sin(t * 0.1 + id * 0.7) * 0.04;
    // Subtle opacity pulse
    mat.opacity = 0.32 + Math.sin(t * 0.6 + id * 1.1) * 0.08;
  });

  return (
    <lineSegments ref={ref} geometry={geometry} position={pos}>
      <lineBasicMaterial color={color} transparent opacity={0.32} blending={THREE.AdditiveBlending} depthWrite={false} />
    </lineSegments>
  );
}

/* ━━━ 3. Ember Particles ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function EmberParticles({ origin, count = 60 }: { origin: [number, number, number]; count?: number }) {
  const ref = useRef<THREE.Points>(null);

  const { offsets, driftX, driftZ } = useMemo(() => {
    const o = new Float32Array(count);
    const dx = new Float32Array(count);
    const dz = new Float32Array(count);
    for (let i = 0; i < count; i++) {
      o[i] = Math.random();
      dx[i] = (Math.random() - 0.5) * 0.5;
      dz[i] = (Math.random() - 0.5) * 0.5;
    }
    return { offsets: o, driftX: dx, driftZ: dz };
  }, [count]);

  const positions = useMemo(() => new Float32Array(count * 3), [count]);
  const colors = useMemo(() => new Float32Array(count * 3), [count]);

  useFrame(({ clock }) => {
    if (!ref.current) return;
    const posAttr = ref.current.geometry.attributes.position as THREE.BufferAttribute;
    const colAttr = ref.current.geometry.attributes.color as THREE.BufferAttribute;
    const pArr = posAttr.array as Float32Array;
    const cArr = colAttr.array as Float32Array;
    const t = clock.getElapsedTime();

    for (let i = 0; i < count; i++) {
      const i3 = i * 3;
      const life = (t * 0.15 + offsets[i]) % 1;
      const fade = 1 - life;
      pArr[i3]     = origin[0] + driftX[i] * life + Math.sin(t * 2 + i) * 0.04 * life;
      pArr[i3 + 1] = origin[1] + life * 2.5;
      pArr[i3 + 2] = origin[2] + driftZ[i] * life + Math.cos(t * 1.5 + i) * 0.03 * life;
      cArr[i3]     = 0.98 * fade;
      cArr[i3 + 1] = 0.45 * fade * fade;
      cArr[i3 + 2] = 0.09 * fade * fade * fade;
    }
    posAttr.needsUpdate = true;
    colAttr.needsUpdate = true;
  });

  return (
    <points ref={ref}>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
        <bufferAttribute attach="attributes-color" args={[colors, 3]} />
      </bufferGeometry>
      <pointsMaterial
        size={0.04} vertexColors transparent opacity={0.7}
        blending={THREE.AdditiveBlending} depthWrite={false} sizeAttenuation
      />
    </points>
  );
}

/* ━━━ 4. Relationship Stream ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function RelStream({ from, to, type }: { from: [number, number, number]; to: [number, number, number]; type: RelType }) {
  const ref = useRef<THREE.Points>(null);
  const count = 25;
  const bidir = type === "peer" || type === "collab";

  const ctrl = useMemo<[number, number, number]>(() => [
    (from[0] + to[0]) / 2,
    (from[1] + to[1]) / 2 + 0.6,
    (from[2] + to[2]) / 2,
  ], [from, to]);

  const positions = useMemo(() => new Float32Array(count * 3), [count]);

  useFrame(({ clock }) => {
    if (!ref.current) return;
    const arr = (ref.current.geometry.attributes.position as THREE.BufferAttribute).array as Float32Array;
    const t = clock.getElapsedTime();

    for (let i = 0; i < count; i++) {
      const i3 = i * 3;
      let p = ((i / count) + t * 0.2) % 1;
      if (bidir && i >= count / 2) p = 1 - (((i / count) + t * 0.2) % 1);
      const q = 1 - p;
      arr[i3]     = q * q * from[0] + 2 * q * p * ctrl[0] + p * p * to[0] + Math.sin(t * 2 + i) * 0.015;
      arr[i3 + 1] = q * q * from[1] + 2 * q * p * ctrl[1] + p * p * to[1];
      arr[i3 + 2] = q * q * from[2] + 2 * q * p * ctrl[2] + p * p * to[2] + Math.cos(t * 2 + i) * 0.015;
    }
    (ref.current.geometry.attributes.position as THREE.BufferAttribute).needsUpdate = true;
  });

  return (
    <points ref={ref}>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
      </bufferGeometry>
      <pointsMaterial
        size={0.05} color={REL_HEX[type]} transparent opacity={0.85}
        blending={THREE.AdditiveBlending} depthWrite={false} sizeAttenuation
      />
    </points>
  );
}

/* ━━━ 5. Priority Pulse ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function PriorityPulse({ origin, entityId }: { origin: [number, number, number]; entityId: number }) {
  const ref = useRef<THREE.Mesh>(null);

  useFrame(({ clock }) => {
    if (!ref.current) return;
    const t = clock.getElapsedTime();
    const cycle = (t % 4) / 4;
    const s = 0.5 + cycle * 3;
    ref.current.scale.set(s, s, s);
    (ref.current.material as THREE.MeshBasicMaterial).opacity = Math.max(0, 0.2 * (1 - cycle));
    ref.current.position.set(
      origin[0] + Math.sin(t * 0.3 + entityId * 2.1) * 0.08,
      origin[1] + Math.sin(t * 0.5 + entityId * 1.3) * 0.15,
      origin[2],
    );
  });

  return (
    <mesh ref={ref} position={origin} rotation={[Math.PI / 2, 0, 0]}>
      <ringGeometry args={[0.8, 0.85, 32]} />
      <meshBasicMaterial
        color="#3b82f6" transparent opacity={0.2} side={THREE.DoubleSide}
        blending={THREE.AdditiveBlending} depthWrite={false}
      />
    </mesh>
  );
}

/* ━━━ 6. Remote Portal ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function RemotePortal() {
  const outerRef = useRef<THREE.Mesh>(null);
  const innerRef = useRef<THREE.Mesh>(null);
  const dotsRef = useRef<THREE.Group>(null);
  const px = 11.5, py = 2.5, pz = -4;

  useFrame(({ clock }) => {
    const t = clock.getElapsedTime();
    if (outerRef.current) {
      outerRef.current.rotation.x = Math.PI / 2 + Math.sin(t * 0.3) * 0.1;
      outerRef.current.rotation.z = t * 0.15;
      (outerRef.current.material as THREE.MeshBasicMaterial).opacity = 0.14 + Math.sin(t * 0.8) * 0.05;
    }
    if (innerRef.current) {
      innerRef.current.rotation.x = Math.PI / 2 + Math.sin(t * 0.3) * 0.1;
      innerRef.current.rotation.z = -t * 0.1;
      (innerRef.current.material as THREE.MeshBasicMaterial).opacity = 0.06 + Math.sin(t * 1.2) * 0.03;
    }
    if (dotsRef.current) {
      dotsRef.current.children.forEach((dot, i) => {
        const angle = t * 0.4 + (i * Math.PI * 2) / 5;
        dot.position.set(px + Math.cos(angle) * 1.1, py + Math.sin(angle) * 0.3, pz + Math.sin(angle) * 1.1);
      });
    }
  });

  return (
    <group>
      <mesh ref={outerRef} position={[px, py, pz]}>
        <torusGeometry args={[1, 0.015, 16, 48]} />
        <meshBasicMaterial color="#6366f1" transparent opacity={0.08} blending={THREE.AdditiveBlending} depthWrite={false} />
      </mesh>
      <mesh ref={innerRef} position={[px, py, pz]}>
        <torusGeometry args={[0.7, 0.008, 16, 48]} />
        <meshBasicMaterial color="#7c3aed" transparent opacity={0.04} blending={THREE.AdditiveBlending} depthWrite={false} />
      </mesh>
      <group ref={dotsRef}>
        {["#ef4444", "#22c55e", "#3b82f6", "#eab308", "#a855f7"].map((c, i) => (
          <mesh key={i} position={[px, py, pz]}>
            <sphereGeometry args={[0.035, 8, 8]} />
            <meshBasicMaterial color={c} transparent opacity={0.6} blending={THREE.AdditiveBlending} />
          </mesh>
        ))}
      </group>
    </group>
  );
}

/* ━━━ 7. Canvas Grid ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function CanvasGrid() {
  const ref = useRef<THREE.LineSegments>(null);
  const geometry = useMemo(() => {
    const geo = new THREE.BufferGeometry();
    const v: number[] = [];
    const size = 24, step = 2;
    for (let z = -size; z <= size; z += step) v.push(-size, 0, z, size, 0, z);
    for (let x = -size; x <= size; x += step) v.push(x, 0, -size, x, 0, size);
    geo.setAttribute("position", new THREE.Float32BufferAttribute(v, 3));
    return geo;
  }, []);

  useFrame(({ clock }) => {
    if (ref.current) ref.current.position.y = -4 + Math.sin(clock.getElapsedTime() * 0.15) * 0.2;
  });

  return (
    <lineSegments ref={ref} geometry={geometry} position={[0, -4, 0]} rotation={[0.3, 0, 0]}>
      <lineBasicMaterial color="#6366f1" transparent opacity={0.1} />
    </lineSegments>
  );
}

/* ━━━ 8. Ambient Particles ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function AmbientParticles() {
  const ref = useRef<THREE.Points>(null);
  const count = 800;

  const [positions, colors] = useMemo(() => {
    const pos = new Float32Array(count * 3);
    const col = new Float32Array(count * 3);
    for (let i = 0; i < count; i++) {
      const i3 = i * 3;
      const r = Math.random() * 20;
      const th = Math.random() * Math.PI * 2;
      const ph = Math.random() * Math.PI;
      pos[i3]     = r * Math.sin(ph) * Math.cos(th);
      pos[i3 + 1] = r * Math.sin(ph) * Math.sin(th);
      pos[i3 + 2] = r * Math.cos(ph);
      const t = Math.random();
      if (t < 0.3)      { col[i3] = 0.39; col[i3 + 1] = 0.40; col[i3 + 2] = 0.95; }
      else if (t < 0.5) { col[i3] = 0.49; col[i3 + 1] = 0.23; col[i3 + 2] = 0.93; }
      else              { col[i3] = 0.28; col[i3 + 1] = 0.32; col[i3 + 2] = 0.40; }
    }
    return [pos, col];
  }, []);

  useFrame(({ clock }) => {
    if (!ref.current) return;
    const t = clock.getElapsedTime();
    ref.current.rotation.y = t * 0.012;
    ref.current.rotation.x = Math.sin(t * 0.03) * 0.03;
  });

  return (
    <points ref={ref}>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
        <bufferAttribute attach="attributes-color" args={[colors, 3]} />
      </bufferGeometry>
      <pointsMaterial
        size={0.03} vertexColors transparent opacity={0.6}
        sizeAttenuation blending={THREE.AdditiveBlending} depthWrite={false}
      />
    </points>
  );
}

/* ━━━ Camera Drift ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function CameraDrift() {
  const { camera } = useThree();
  useFrame(({ clock }) => {
    const t = clock.getElapsedTime();
    camera.position.x = Math.sin(t * 0.08) * 0.3;
    camera.position.y = 1 + Math.sin(t * 0.06) * 0.2;
    camera.lookAt(0, 0, 0);
  });
  return null;
}

/* ━━━ Scene Composition ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function Scene() {
  return (
    <>
      {/* Lighting — spread wider */}
      <ambientLight intensity={0.2} />
      <pointLight position={[12, 5, 4]} intensity={0.5} color="#6366f1" />
      <pointLight position={[-12, -3, -5]} intensity={0.4} color="#7c3aed" />
      <pointLight position={[0, 4, 2]} intensity={0.2} color="#3b82f6" />
      <pointLight position={[-10, 2, 2]} intensity={0.3} color="#eab308" />
      <pointLight position={[10, -2, 3]} intensity={0.25} color="#06b6d4" />

      {/* 14 session entities */}
      {ENTITIES.map(e => <SessionEntity key={e.id} e={e} />)}

      {/* 6 floating terminal window frames */}
      {TERMINALS.map(t => (
        <TerminalFrame key={t.id} pos={t.pos} w={t.w} h={t.h} color={t.color} id={t.id} />
      ))}

      {/* Embers from fire entity (left) + light embers from running entity (right) */}
      <EmberParticles origin={ENTITIES[0].pos} count={60} />
      <EmberParticles origin={ENTITIES[7].pos} count={25} />
      <EmberParticles origin={ENTITIES[17].pos} count={40} />

      {/* 8 relationship streams */}
      {RELS.map((r, i) => (
        <RelStream key={i} from={ENTITIES[r.from].pos} to={ENTITIES[r.to].pos} type={r.type} />
      ))}

      {/* Priority pulse from the waiting entity (right side) */}
      <PriorityPulse origin={ENTITIES[5].pos} entityId={5} />

      {/* Remote collaboration portal — far upper right */}
      <RemotePortal />

      {/* Canvas grid floor */}
      <CanvasGrid />

      {/* Background star field */}
      <AmbientParticles />

      {/* Camera drift */}
      <CameraDrift />
    </>
  );
}

/* ━━━ Export ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

export function HeroBg() {
  const [mounted, setMounted] = useState(false);
  useEffect(() => { setMounted(true); }, []);
  if (!mounted) return null;

  return (
    <div style={{ position: "absolute", inset: 0, zIndex: 0, pointerEvents: "none" }}>
      <Canvas
        dpr={[1, 1.5]}
        camera={{ position: [0, 1, 8], fov: 80 }}
        style={{ background: "transparent" }}
        gl={{ alpha: true, antialias: false, powerPreference: "high-performance" }}
      >
        <Scene />
      </Canvas>
    </div>
  );
}
