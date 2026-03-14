"use client";

import { useRef, useMemo, useCallback, useState, useEffect } from "react";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import * as THREE from "three";

function ParticleField() {
  const ref = useRef<THREE.Points>(null);
  const count = 1200;

  const [positions, colors] = useMemo(() => {
    const pos = new Float32Array(count * 3);
    const col = new Float32Array(count * 3);

    // Indigo: #6366f1 → (0.388, 0.4, 0.945)
    // Violet: #7c3aed → (0.486, 0.227, 0.929)
    // Slate:  #94a3b8 → (0.58, 0.64, 0.72)

    for (let i = 0; i < count; i++) {
      const i3 = i * 3;
      const radius = Math.random() * 12;
      const theta = Math.random() * Math.PI * 2;
      const phi = Math.random() * Math.PI;

      pos[i3] = radius * Math.sin(phi) * Math.cos(theta);
      pos[i3 + 1] = radius * Math.sin(phi) * Math.sin(theta);
      pos[i3 + 2] = radius * Math.cos(phi);

      const t = Math.random();
      if (t < 0.4) {
        // Indigo particles
        col[i3] = 0.388;
        col[i3 + 1] = 0.4;
        col[i3 + 2] = 0.945;
      } else if (t < 0.7) {
        // Violet particles
        col[i3] = 0.486;
        col[i3 + 1] = 0.227;
        col[i3 + 2] = 0.929;
      } else {
        // Slate/dim particles
        col[i3] = 0.36;
        col[i3 + 1] = 0.4;
        col[i3 + 2] = 0.52;
      }
    }
    return [pos, col];
  }, []);

  useFrame(({ clock }) => {
    if (!ref.current) return;
    const t = clock.getElapsedTime();
    ref.current.rotation.y = t * 0.03;
    ref.current.rotation.x = Math.sin(t * 0.08) * 0.08;
  });

  return (
    <points ref={ref}>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
        <bufferAttribute attach="attributes-color" args={[colors, 3]} />
      </bufferGeometry>
      <pointsMaterial
        size={0.025}
        vertexColors
        transparent
        opacity={0.6}
        sizeAttenuation
        blending={THREE.AdditiveBlending}
        depthWrite={false}
      />
    </points>
  );
}

function FloatingGrid() {
  const ref = useRef<THREE.LineSegments>(null);

  const geometry = useMemo(() => {
    const geo = new THREE.BufferGeometry();
    const verts: number[] = [];
    const gridSize = 20;
    const step = 2;

    // Horizontal lines
    for (let z = -gridSize; z <= gridSize; z += step) {
      verts.push(-gridSize, 0, z, gridSize, 0, z);
    }
    // Vertical lines
    for (let x = -gridSize; x <= gridSize; x += step) {
      verts.push(x, 0, -gridSize, x, 0, gridSize);
    }

    geo.setAttribute("position", new THREE.Float32BufferAttribute(verts, 3));
    return geo;
  }, []);

  useFrame(({ clock }) => {
    if (!ref.current) return;
    const t = clock.getElapsedTime();
    ref.current.position.y = -4 + Math.sin(t * 0.2) * 0.3;
  });

  return (
    <lineSegments ref={ref} geometry={geometry} position={[0, -4, 0]} rotation={[0.3, 0, 0]}>
      <lineBasicMaterial color="#6366f1" transparent opacity={0.06} />
    </lineSegments>
  );
}

function GlowOrb() {
  const ref = useRef<THREE.Mesh>(null);
  const { pointer, viewport } = useThree();

  useFrame(({ clock }, delta) => {
    if (!ref.current) return;
    const t = clock.getElapsedTime();

    // Gently follow cursor with lag
    const tx = (pointer.x * viewport.width) / 4;
    const ty = (pointer.y * viewport.height) / 4;
    ref.current.position.x += (tx - ref.current.position.x) * 0.02;
    ref.current.position.y += (ty - ref.current.position.y) * 0.02;

    // Slow rotation
    ref.current.rotation.x += delta * 0.15;
    ref.current.rotation.y += delta * 0.1;

    // Pulsing scale
    const s = 1 + Math.sin(t * 0.8) * 0.1;
    ref.current.scale.setScalar(s);
  });

  return (
    <mesh ref={ref} position={[0, 0, -2]}>
      <icosahedronGeometry args={[1.5, 3]} />
      <meshBasicMaterial
        color="#6366f1"
        transparent
        opacity={0.04}
        wireframe
      />
    </mesh>
  );
}

function Scene() {
  return (
    <>
      <ambientLight intensity={0.3} />
      <pointLight position={[8, 8, 8]} intensity={0.6} color="#6366f1" />
      <pointLight position={[-8, -4, -8]} intensity={0.3} color="#7c3aed" />
      <ParticleField />
      <FloatingGrid />
      <GlowOrb />
    </>
  );
}

export function HeroBg() {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) return null;

  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        zIndex: 0,
        pointerEvents: "none",
      }}
    >
      <Canvas
        dpr={[1, 1.5]}
        camera={{ position: [0, 0, 8], fov: 60 }}
        style={{ background: "transparent" }}
        gl={{ alpha: true, antialias: false, powerPreference: "high-performance" }}
      >
        <Scene />
      </Canvas>
    </div>
  );
}
