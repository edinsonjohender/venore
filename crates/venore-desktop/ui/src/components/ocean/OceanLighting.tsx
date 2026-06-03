// =============================================================================
// OceanLighting — Ambient + directional light (no shadows)
// =============================================================================

export function OceanLighting() {
  return (
    <>
      <ambientLight intensity={0.4} />
      <directionalLight position={[50, 80, 50]} intensity={0.6} />
    </>
  )
}
