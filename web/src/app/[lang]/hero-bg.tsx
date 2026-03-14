"use client";

import { useRef, useMemo, useState, useEffect } from "react";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import * as THREE from "three";

/* ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   "The Living Workshop" — Agent Hand Hero Background

   Visual narrative: AI agent sessions as living geometric entities
   floating in a digital workspace. Each entity has personality through
   its shape, color, and animation — reflecting its real status in
   the Agent Hand TUI.

   Elements:
   1. Session entities  — wireframe polyhedra with status animations
   2. Ember particles   — rising from "on fire" busy sessions
   3. Relationship beams — flowing particles between connected sessions
   4. Priority pulse     — expanding ring from attention-seeking sessions
   5. Remote portal      — torus gateway with orbiting presence dots
   6. Canvas grid        — the workflow editor's infinite canvas floor
   7. Ambient particles  — background star field in brand colors
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

// ─── Scene Data (positioned to leave centre open for hero text) ───

const ENTITIES: Entity[] = [
  { id: 0, pos: [-3.0,  1.2,  0  ], status: "fire",     tool: "claude", size: 0.55, geo: "ico"    },
  { id: 1, pos: [ 2.5,  1.0, -0.8], status: "waiting",  tool: "claude", size: 0.45, geo: "dodeca" },
  { id: 2, pos: [-1.0, -2.0,  0.3], status: "idle",     tool: "shell",  size: 0.35, geo: "ico"    },
  { id: 3, pos: [ 3.8, -0.5, -0.5], status: "running",  tool: "gemini", size: 0.42, geo: "dodeca" },
  { id: 4, pos: [ 0.5,  2.5, -2  ], status: "idle",     tool: "claude", size: 0.3,  geo: "ico"    },
  { id: 5, pos: [-3.8, -0.8, -1  ], status: "starting", tool: "shell",  size: 0.38, geo: "octa"   },
  { id: 6, pos: [ 1.0, -2.5,  0.5], status: "running",  tool: "claude", size: 0.4,  geo: "ico"    },
];

const RELS: { from: number; to: number; type: RelType }[] = [
  { from: 0, to: 6, type: "parent"     },   // Claude spawned a child
  { from: 0, to: 1, type: "dependency"  },   // fire depends on waiting's output
  { from: 3, to: 4, type: "peer"        },   // Gemini ↔ Claude peer work
  { from: 1, to: 2, type: "collab"      },   // active collaboration
];

// ─── Geometry Helper ───

function EntityGeo({ geo, size }: { geo: Geo; size: number }) {
  switch (geo) {
    case "dodeca": return <dodecahedronGeometry args={[size, 0]} />;
    case "octa":   return <octahedronGeometry args={[size, 0]} />;
    default:       return <icosahedronGeometry args={[size, 1]} />;
  }
}

/* ━━━ 1. Session Entity ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   Each entity = wireframe polyhedron + outer glow shell.
   Animation varies by status:
     fire    → fast spin + vibration + periodic flare-ups
     running → moderate spin + gentle pulse
     waiting → slow drift + blink (opacity oscillation)
     starting→ emerging animation (scale + fade in/out)
     idle    → barely moving, dim
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function SessionEntity({ e }: { e: Entity }) {
  const meshRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.Mesh>(null);

  useFrame(({ clock }) => {
    const mesh = meshRef.current;
    if (!mesh) return;
    const t = clock.getElapsedTime();
    const mat = mesh.material as THREE.MeshBasicMaterial;

    // Gentle floating (all entities)
    mesh.position.x = e.pos[0] + Math.sin(t * 0.3 + e.id * 2.1) * 0.08;
    mesh.position.y = e.pos[1] + Math.sin(t * 0.5 + e.id * 1.3) * 0.15;
    mesh.position.z = e.pos[2];

    switch (e.status) {
      case "fire": {
        mesh.rotation.x += 0.015;
        mesh.rotation.y += 0.02;
        // Vibration — overheating!
        mesh.position.x += Math.sin(t * 15 + e.id) * 0.02;
        mesh.position.y += Math.cos(t * 12 + e.id) * 0.015;
        // Periodic flare-up every ~8s
        const flare = Math.pow(Math.max(0, Math.sin(t * 0.4)), 20);
        mat.opacity = 0.45 + Math.sin(t * 4) * 0.1 + flare * 0.3;
        mesh.scale.setScalar(1 + Math.sin(t * 1.5) * 0.08 + flare * 0.2);
        break;
      }
      case "running":
        mesh.rotation.x += 0.008;
        mesh.rotation.y += 0.012;
        mat.opacity = 0.4;
        mesh.scale.setScalar(1 + Math.sin(t * 1.2 + e.id) * 0.05);
        break;
      case "waiting":
        mesh.rotation.y += 0.003;
        mat.opacity = 0.2 + Math.abs(Math.sin(t * 2)) * 0.4;
        break;
      case "starting":
        mesh.rotation.x += 0.01;
        mesh.rotation.z += 0.015;
        mat.opacity = 0.15 + Math.abs(Math.sin(t * 3)) * 0.45;
        mesh.scale.setScalar(0.8 + Math.sin(t * 2) * 0.15);
        break;
      default: // idle
        mesh.rotation.y += 0.002;
        mat.opacity = 0.2;
    }

    // Sync outer glow shell
    const glow = glowRef.current;
    if (glow) {
      glow.position.copy(mesh.position);
      glow.rotation.copy(mesh.rotation);
      glow.scale.setScalar(mesh.scale.x * 1.8);
      const gMat = glow.material as THREE.MeshBasicMaterial;
      gMat.opacity = e.status === "fire"    ? 0.03 + Math.sin(t * 3) * 0.02
                   : e.status === "running" ? 0.025
                   : 0.015;
    }
  });

  return (
    <group>
      <mesh ref={meshRef} position={e.pos}>
        <EntityGeo geo={e.geo} size={e.size} />
        <meshBasicMaterial color={STATUS_HEX[e.status]} wireframe transparent opacity={0.3} />
      </mesh>
      <mesh ref={glowRef} position={e.pos}>
        <EntityGeo geo={e.geo} size={e.size * 1.8} />
        <meshBasicMaterial
          color={TOOL_HEX[e.tool]} wireframe transparent opacity={0.02}
          blending={THREE.AdditiveBlending} depthWrite={false}
        />
      </mesh>
    </group>
  );
}

/* ━━━ 2. Ember Particles ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   Rising embers from "on fire" entities.
   Particles start orange, shift to red, then fade to dark (additive).
   Continuous cycle — each particle has a staggered phase offset.
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

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

      // Position: rise upward with slight lateral drift
      pArr[i3]     = origin[0] + driftX[i] * life + Math.sin(t * 2 + i) * 0.04 * life;
      pArr[i3 + 1] = origin[1] + life * 2.5;
      pArr[i3 + 2] = origin[2] + driftZ[i] * life + Math.cos(t * 1.5 + i) * 0.03 * life;

      // Colour: orange → red → dark (channels fade at different rates)
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

/* ━━━ 3. Relationship Stream ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   Flowing particles along a quadratic bezier curve between entities.
   Colour by relationship type. Bidirectional types (peer, collab) have
   particles flowing both ways.
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

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

      // Quadratic bezier interpolation
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
        size={0.03} color={REL_HEX[type]} transparent opacity={0.5}
        blending={THREE.AdditiveBlending} depthWrite={false} sizeAttenuation
      />
    </points>
  );
}

/* ━━━ 4. Priority Pulse ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   Expanding ring from the "waiting" entity — the Ctrl+N jump signal.
   Blue ring expands outward and fades every 4 seconds.
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function PriorityPulse({ origin }: { origin: [number, number, number] }) {
  const ref = useRef<THREE.Mesh>(null);

  useFrame(({ clock }) => {
    if (!ref.current) return;
    const t = clock.getElapsedTime();
    const cycle = (t % 4) / 4;
    const s = 0.5 + cycle * 3;
    ref.current.scale.set(s, s, s);
    (ref.current.material as THREE.MeshBasicMaterial).opacity = Math.max(0, 0.2 * (1 - cycle));
    // Track entity float
    ref.current.position.set(
      origin[0] + Math.sin(t * 0.3 + 1 * 2.1) * 0.08,
      origin[1] + Math.sin(t * 0.5 + 1 * 1.3) * 0.15,
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

/* ━━━ 5. Remote Portal ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   A torus gateway in the distance with orbiting coloured presence dots
   representing remote collaborators. Inner ring rotates counter to outer.
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function RemotePortal() {
  const outerRef = useRef<THREE.Mesh>(null);
  const innerRef = useRef<THREE.Mesh>(null);
  const dotsRef = useRef<THREE.Group>(null);
  const px = 4.5, py = 1.2, pz = -3.5;

  useFrame(({ clock }) => {
    const t = clock.getElapsedTime();
    if (outerRef.current) {
      outerRef.current.rotation.x = Math.PI / 2 + Math.sin(t * 0.3) * 0.1;
      outerRef.current.rotation.z = t * 0.15;
      (outerRef.current.material as THREE.MeshBasicMaterial).opacity = 0.08 + Math.sin(t * 0.8) * 0.03;
    }
    if (innerRef.current) {
      innerRef.current.rotation.x = Math.PI / 2 + Math.sin(t * 0.3) * 0.1;
      innerRef.current.rotation.z = -t * 0.1;
      (innerRef.current.material as THREE.MeshBasicMaterial).opacity = 0.03 + Math.sin(t * 1.2) * 0.015;
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

/* ━━━ 6. Canvas Grid ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   The workflow editor's infinite canvas floor — faint indigo grid lines
   gently oscillating below the scene.
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function CanvasGrid() {
  const ref = useRef<THREE.LineSegments>(null);
  const geometry = useMemo(() => {
    const geo = new THREE.BufferGeometry();
    const v: number[] = [];
    const size = 20, step = 2;
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
      <lineBasicMaterial color="#6366f1" transparent opacity={0.04} />
    </lineSegments>
  );
}

/* ━━━ 7. Ambient Particles ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   Sparse star field in brand colours (indigo / violet / dim slate).
   Slowly rotates to give depth to the scene.
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function AmbientParticles() {
  const ref = useRef<THREE.Points>(null);
  const count = 600;

  const [positions, colors] = useMemo(() => {
    const pos = new Float32Array(count * 3);
    const col = new Float32Array(count * 3);
    for (let i = 0; i < count; i++) {
      const i3 = i * 3;
      const r = Math.random() * 14;
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
    ref.current.rotation.y = t * 0.015;
    ref.current.rotation.x = Math.sin(t * 0.04) * 0.04;
  });

  return (
    <points ref={ref}>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
        <bufferAttribute attach="attributes-color" args={[colors, 3]} />
      </bufferGeometry>
      <pointsMaterial
        size={0.02} vertexColors transparent opacity={0.35}
        sizeAttenuation blending={THREE.AdditiveBlending} depthWrite={false}
      />
    </points>
  );
}

/* ━━━ Camera Drift ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   Very slow sine-based camera movement to make the scene feel alive.
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function CameraDrift() {
  const { camera } = useThree();
  useFrame(({ clock }) => {
    const t = clock.getElapsedTime();
    camera.position.x = Math.sin(t * 0.08) * 0.4;
    camera.position.y = 1 + Math.sin(t * 0.06) * 0.25;
    camera.lookAt(0, 0, 0);
  });
  return null;
}

/* ━━━ Scene Composition ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ */

function Scene() {
  return (
    <>
      {/* Subtle lighting */}
      <ambientLight intensity={0.12} />
      <pointLight position={[6, 5, 4]} intensity={0.35} color="#6366f1" />
      <pointLight position={[-5, -3, -5]} intensity={0.2} color="#7c3aed" />
      <pointLight position={[0, 3, 2]} intensity={0.12} color="#3b82f6" />

      {/* Session entities — the "characters" */}
      {ENTITIES.map(e => <SessionEntity key={e.id} e={e} />)}

      {/* Ember particles rising from the "on fire" entity */}
      <EmberParticles origin={ENTITIES[0].pos} count={60} />

      {/* Relationship beams — energy flowing between connected entities */}
      {RELS.map((r, i) => (
        <RelStream key={i} from={ENTITIES[r.from].pos} to={ENTITIES[r.to].pos} type={r.type} />
      ))}

      {/* Priority pulse — the Ctrl+N attention signal */}
      <PriorityPulse origin={ENTITIES[1].pos} />

      {/* Remote collaboration portal */}
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
        camera={{ position: [0, 1, 8], fov: 55 }}
        style={{ background: "transparent" }}
        gl={{ alpha: true, antialias: false, powerPreference: "high-performance" }}
      >
        <Scene />
      </Canvas>
    </div>
  );
}
