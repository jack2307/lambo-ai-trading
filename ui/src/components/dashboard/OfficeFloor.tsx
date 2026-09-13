/**
 * The company as an open-plan floor.
 *
 * One floor, no walls: each department is a cluster of desks on its own rug,
 * and the only glass on the floor is around the arbiter's corner office. The
 * thing that moves is a *hypothesis*: a lit file that leaves the archive, is
 * written up at the strategy desk, run in the engine bay, and then passes
 * the three veto desks in a row — any one of which can stamp it and send it
 * back to the archive as closed. What passes all three goes through the
 * glass to the arbiter, and from there to the archive as a record. The
 * advisory desks send their notes to the office while a file is on the desk.
 *
 * The clusters are the research loop's steps (`.claude/skills/research/SKILL.md`)
 * and the figures are the agents (`.claude/agents/`). A department with no
 * agent — data, engine, archive — is drawn with its equipment.
 *
 * Plain three.js in a ref; every resource disposed once. Labels are DOM,
 * projected. Colours are the CSS tokens, read at mount; the only colours made
 * in code are those tokens lightened or darkened.
 */

import { useEffect, useRef } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js'

export type Tone = 'arbiter' | 'veto' | 'advisory' | 'ops' | 'public'

export interface Occupant {
  id: string
  title: string
  model: string
}

export interface Department {
  id: string
  title: string
  /** One line: what this cluster does. */
  line: string
  tone: Tone
  /** Centre of the cluster on the floor and the rug's size. */
  x: number
  z: number
  w: number
  d: number
  /** Glass partition around the cluster. Only the arbiter's office has one. */
  enclosed?: boolean
  occupants: Occupant[]
  /** Desks in one row facing the aisle, or two rows facing each other. */
  arrange?: 'row' | 'grid'
  furniture?: 'racks' | 'engine' | 'shelves' | 'lounge' | 'meeting'
  /** One rack per feed, named on the floor in front of it. */
  racks?: string[]
}

interface Props {
  departments: Department[]
  /**
   * Whether the scene moves on its own: the camera drifts, the file walks,
   * the rack lights blink. The page decides this from the system's
   * reduced-motion setting and an explicit override; the scene only obeys.
   */
  animate: boolean
  className?: string
}

const FLOOR_W = 24
/** Deep enough for two wings and the lobby along the south edge. */
const FLOOR_D = 17
const AISLE_X0 = -8.4
const AISLE_X1 = 10.4
const GLASS_H = 1.35

function token(name: string, fallback: string): string {
  if (typeof window === 'undefined') return fallback
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback
}

function glowTexture(): THREE.Texture {
  const size = 128
  const canvas = document.createElement('canvas')
  canvas.width = size
  canvas.height = size
  const ctx = canvas.getContext('2d')!
  const gradient = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2)
  gradient.addColorStop(0, 'rgba(255,255,255,1)')
  gradient.addColorStop(0.28, 'rgba(255,255,255,0.45)')
  gradient.addColorStop(1, 'rgba(255,255,255,0)')
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, size, size)
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  return texture
}

/** A rounded rectangle lying in the x/z plane. */
function rug(w: number, d: number, r = 0.35): THREE.ShapeGeometry {
  const shape = new THREE.Shape()
  const x = -w / 2
  const y = -d / 2
  shape.moveTo(x + r, y)
  shape.lineTo(x + w - r, y)
  shape.quadraticCurveTo(x + w, y, x + w, y + r)
  shape.lineTo(x + w, y + d - r)
  shape.quadraticCurveTo(x + w, y + d, x + w - r, y + d)
  shape.lineTo(x + r, y + d)
  shape.quadraticCurveTo(x, y + d, x, y + d - r)
  shape.lineTo(x, y + r)
  shape.quadraticCurveTo(x, y, x + r, y)
  const geometry = new THREE.ShapeGeometry(shape, 8)
  geometry.rotateX(-Math.PI / 2)
  return geometry
}

interface Cluster {
  dept: Department
  centre: THREE.Vector3
  /** Where the aisle meets the cluster. */
  aisle: THREE.Vector3
  rugMaterial: THREE.MeshPhysicalMaterial
  label: HTMLDivElement
  figures: Figure[]
  glow: number
  glowColor: THREE.Color
}

interface Figure {
  material: THREE.MeshPhysicalMaterial
  glow: number
  glowColor: THREE.Color
}

interface Ring {
  mesh: THREE.Mesh<THREE.RingGeometry, THREE.MeshBasicMaterial>
  age: number
  life: number
  reach: number
}

interface Arc {
  curve: THREE.QuadraticBezierCurve3
  t: number
  duration: number
  head: THREE.Sprite
  tail: THREE.Line<THREE.BufferGeometry, THREE.LineBasicMaterial>
  to: Cluster
}

interface Stop {
  cluster: Cluster
  dwell: number
  vetoChance: number
}

/** One activity light on a rack: flips on and off at its own rate. */
interface Led {
  material: THREE.MeshBasicMaterial
  /** A halo that shows when the light is on; sprites face the camera, so it reads from any angle. */
  halo: THREE.Sprite
  on: THREE.Color
  off: THREE.Color
  /** Expected flips per second. */
  rate: number
  lit: boolean
}

export function OfficeFloor({ departments, animate, className }: Props) {
  const container = useRef<HTMLDivElement>(null)
  const labels = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const host = container.current
    const labelHost = labels.current
    if (!host || !labelHost) return

    const reduceMotion = !animate

    const colors = {
      card: new THREE.Color(token('--card', '#131413')),
      background: new THREE.Color(token('--background', '#0a0a0a')),
      primary: new THREE.Color(token('--primary', '#8dff08')),
      mint: new THREE.Color(token('--brand-mint', '#08ffb5')),
      caution: new THREE.Color(token('--caution', '#d9b04a')),
      foreground: new THREE.Color(token('--foreground', '#ffffff')),
      muted: new THREE.Color(token('--muted-foreground', '#9aa39a')),
      border: new THREE.Color(token('--border', '#242724')),
      elevated: new THREE.Color(token('--elevated', '#1a1c1a')),
    }
    const toneColor = (tone: Tone) =>
      tone === 'arbiter'
        ? colors.primary
        : tone === 'veto'
          ? colors.caution
          : tone === 'ops'
            ? colors.mint
            : tone === 'public'
              ? colors.muted.clone().lerp(colors.foreground, 0.25)
              : colors.foreground.clone().lerp(colors.muted, 0.3)

    /* ---- renderer ---- */
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, powerPreference: 'low-power' })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    renderer.setClearColor(0x000000, 0)
    renderer.outputColorSpace = THREE.SRGBColorSpace
    renderer.toneMapping = THREE.ACESFilmicToneMapping
    renderer.toneMappingExposure = 1.1
    renderer.shadowMap.enabled = true
    renderer.shadowMap.type = THREE.PCFSoftShadowMap
    host.appendChild(renderer.domElement)

    const scene = new THREE.Scene()
    scene.fog = new THREE.Fog(colors.card, 34, 60)

    const pmrem = new THREE.PMREMGenerator(renderer)
    scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture
    scene.environmentIntensity = 0.6
    pmrem.dispose()

    const camera = new THREE.PerspectiveCamera(28, 1, 0.1, 120)
    camera.position.set(17, 21, 31)

    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableZoom = false
    controls.enablePan = false
    controls.enableDamping = true
    controls.dampingFactor = 0.06
    controls.autoRotate = !reduceMotion
    controls.autoRotateSpeed = 0.12
    controls.minPolarAngle = Math.PI * 0.2
    controls.maxPolarAngle = Math.PI * 0.36
    controls.target.set(0, 0.2, 1.4)

    /* ---- light: a warm key as if from a window wall, a cool fill ---- */
    scene.add(new THREE.HemisphereLight(colors.foreground, colors.background, 0.45))
    const key = new THREE.DirectionalLight(colors.foreground.clone().lerp(colors.caution, 0.18), 1.8)
    key.position.set(-10, 15, 8)
    key.castShadow = true
    key.shadow.mapSize.set(2048, 2048)
    key.shadow.camera.left = -17
    key.shadow.camera.right = 17
    key.shadow.camera.top = 17
    key.shadow.camera.bottom = -17
    key.shadow.camera.near = 1
    key.shadow.camera.far = 50
    key.shadow.bias = -0.0004
    key.shadow.radius = 5
    scene.add(key)
    const fill = new THREE.DirectionalLight(colors.mint, 0.25)
    fill.position.set(12, 6, -14)
    scene.add(fill)

    /* ---- floor: a warm timber field on a dark slab, with one aisle ---- */
    const timber = colors.elevated.clone().lerp(colors.caution, 0.22).lerp(colors.foreground, 0.05)
    const slab = new THREE.Mesh(
      new THREE.BoxGeometry(FLOOR_W + 1.2, 0.22, FLOOR_D + 1.2),
      new THREE.MeshPhysicalMaterial({ color: colors.background, roughness: 0.5, clearcoat: 0.4, clearcoatRoughness: 0.4 }),
    )
    slab.position.y = -0.12
    slab.receiveShadow = true
    scene.add(slab)
    const floor = new THREE.Mesh(
      new THREE.PlaneGeometry(FLOOR_W, FLOOR_D),
      new THREE.MeshPhysicalMaterial({ color: timber, roughness: 0.42, clearcoat: 0.9, clearcoatRoughness: 0.28 }),
    )
    floor.rotation.x = -Math.PI / 2
    floor.receiveShadow = true
    scene.add(floor)
    // Plank seams, so the field reads as timber and not paint.
    const seam = new THREE.MeshBasicMaterial({ color: timber.clone().lerp(colors.background, 0.35) })
    for (let z = -FLOOR_D / 2 + 0.6; z < FLOOR_D / 2; z += 0.6) {
      const line = new THREE.Mesh(new THREE.PlaneGeometry(FLOOR_W, 0.012), seam)
      line.rotation.x = -Math.PI / 2
      line.position.set(0, 0.002, z)
      scene.add(line)
    }
    const aisle = new THREE.Mesh(
      new THREE.PlaneGeometry(AISLE_X1 - AISLE_X0, 1.2),
      new THREE.MeshPhysicalMaterial({ color: colors.elevated.clone().lerp(colors.foreground, 0.1), roughness: 0.35, clearcoat: 0.8, clearcoatRoughness: 0.2 }),
    )
    aisle.rotation.x = -Math.PI / 2
    aisle.position.set((AISLE_X0 + AISLE_X1) / 2, 0.004, 0)
    aisle.receiveShadow = true
    scene.add(aisle)

    /* ---- shared materials ---- */
    const glass = new THREE.MeshPhysicalMaterial({
      color: colors.foreground.clone().lerp(colors.mint, 0.2),
      transparent: true,
      opacity: 0.12,
      roughness: 0.05,
      clearcoat: 1,
      side: THREE.DoubleSide,
      depthWrite: false,
    })
    const frameMaterial = new THREE.MeshPhysicalMaterial({ color: colors.border.clone().lerp(colors.foreground, 0.45), roughness: 0.3, metalness: 0.5 })
    const deskTop = new THREE.MeshPhysicalMaterial({ color: colors.foreground.clone().lerp(colors.elevated, 0.55), roughness: 0.35, clearcoat: 0.7, clearcoatRoughness: 0.2 })
    const deskLeg = new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.foreground, 0.2), roughness: 0.4, metalness: 0.5 })
    const chairMaterial = new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.foreground, 0.16), roughness: 0.6 })
    const screenMaterial = new THREE.MeshPhysicalMaterial({ color: colors.background, roughness: 0.15, emissive: colors.mint.clone(), emissiveIntensity: 0.4 })
    const equipment = new THREE.MeshPhysicalMaterial({ color: colors.elevated.clone().lerp(colors.foreground, 0.14), roughness: 0.45, metalness: 0.35 })
    const glow = glowTexture()
    const haloFor = (color: THREE.Color, size: number) => {
      const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: glow, color, transparent: true, opacity: 0, blending: THREE.AdditiveBlending, depthWrite: false }))
      sprite.scale.setScalar(size)
      scene.add(sprite)
      return sprite
    }
    const rackUnit = new THREE.MeshPhysicalMaterial({ color: colors.elevated.clone().lerp(colors.foreground, 0.3), roughness: 0.5, metalness: 0.4 })
    const ledGeometry = new THREE.BoxGeometry(0.06, 0.035, 0.012)
    const leds: Led[] = []
    const rackTags: { label: HTMLDivElement; at: THREE.Vector3 }[] = []
    let showCar: THREE.Group | null = null
    const leaf = new THREE.MeshPhysicalMaterial({ color: colors.mint.clone().lerp(colors.background, 0.55), roughness: 0.7 })
    const pot = new THREE.MeshPhysicalMaterial({ color: colors.elevated.clone().lerp(colors.caution, 0.12), roughness: 0.6 })

    const shadowed = (mesh: THREE.Mesh) => {
      mesh.castShadow = true
      mesh.receiveShadow = true
      return mesh
    }

    /** A chair at (x, z) whose back is on the −facing side. */
    const chair = (x: number, z: number, facing: 1 | -1) => {
      const seat = shadowed(new THREE.Mesh(new THREE.CylinderGeometry(0.22, 0.22, 0.06, 24), chairMaterial))
      seat.position.set(x, 0.45, z)
      scene.add(seat)
      const back = shadowed(new THREE.Mesh(new THREE.BoxGeometry(0.42, 0.42, 0.05), chairMaterial))
      back.position.set(x, 0.72, z - facing * 0.2)
      scene.add(back)
      const post = new THREE.Mesh(new THREE.CylinderGeometry(0.03, 0.03, 0.42, 12), deskLeg)
      post.position.set(x, 0.22, z)
      scene.add(post)
    }

    /** A desk with a monitor and a chair; the person sits facing `facing` along z. */
    const desk = (x: number, z: number, facing: 1 | -1, material: THREE.Material) => {
      const top = shadowed(new THREE.Mesh(new THREE.BoxGeometry(1.1, 0.05, 0.55), deskTop))
      top.position.set(x, 0.72, z)
      scene.add(top)
      for (const dx of [-0.48, 0.48]) {
        const legMesh = new THREE.Mesh(new THREE.BoxGeometry(0.05, 0.7, 0.5), deskLeg)
        legMesh.position.set(x + dx, 0.35, z)
        scene.add(legMesh)
      }
      const screen = new THREE.Mesh(new THREE.BoxGeometry(0.42, 0.26, 0.025), screenMaterial)
      screen.position.set(x, 0.92, z + facing * 0.16)
      scene.add(screen)
      const stand = new THREE.Mesh(new THREE.BoxGeometry(0.05, 0.14, 0.05), deskLeg)
      stand.position.set(x, 0.79, z + facing * 0.16)
      scene.add(stand)
      // The chair, on the near side of the desk.
      const cz = z - facing * 0.62
      chair(x, cz, facing)
      // The person.
      const body = shadowed(new THREE.Mesh(new THREE.CapsuleGeometry(0.17, 0.3, 6, 18), material))
      body.position.set(x, 0.76, cz + facing * 0.02)
      scene.add(body)
      const head = shadowed(new THREE.Mesh(new THREE.SphereGeometry(0.135, 20, 20), material))
      head.position.set(x, 1.16, cz + facing * 0.02)
      scene.add(head)
    }

    /**
     * A show car: a single-seater built from primitives, nose to the +x side.
     * Papaya body (the caution token), dark tyres, a wing at each end, a halo
     * over the cockpit. Returned as a group so the plinth can turn it.
     */
    const raceCar = (): THREE.Group => {
      const g = new THREE.Group()
      const body = new THREE.MeshPhysicalMaterial({ color: colors.caution.clone().lerp(colors.foreground, 0.05), roughness: 0.25, metalness: 0.2, clearcoat: 1, clearcoatRoughness: 0.08 })
      const dark = new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.foreground, 0.08), roughness: 0.55 })
      const tyre = new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.foreground, 0.05), roughness: 0.9 })
      const rim = new THREE.MeshPhysicalMaterial({ color: colors.foreground.clone().lerp(colors.muted, 0.4), roughness: 0.3, metalness: 0.8 })
      const add = (geometry: THREE.BufferGeometry, material: THREE.Material, x: number, y: number, z: number, rx = 0, ry = 0, rz = 0) => {
        const m = shadowed(new THREE.Mesh(geometry, material))
        m.position.set(x, y, z)
        m.rotation.set(rx, ry, rz)
        g.add(m)
        return m
      }
      // Monocoque: a tapered tub, a nose cone, the engine cover behind the seat.
      add(new THREE.BoxGeometry(1.5, 0.16, 0.34), body, 0.05, 0.2, 0)
      add(new THREE.CylinderGeometry(0.02, 0.15, 0.7, 20), body, 1.1, 0.19, 0, 0, 0, -Math.PI / 2)
      add(new THREE.BoxGeometry(0.7, 0.14, 0.26), body, -0.55, 0.32, 0)
      add(new THREE.BoxGeometry(0.28, 0.1, 0.08), dark, -0.85, 0.4, 0) // airbox
      // Sidepods and the floor edges.
      for (const s of [-1, 1]) {
        add(new THREE.BoxGeometry(0.85, 0.14, 0.2), body, -0.25, 0.19, s * 0.27)
        add(new THREE.BoxGeometry(1.7, 0.02, 0.12), dark, 0.05, 0.11, s * 0.36)
      }
      // Cockpit opening, headrest and the halo.
      add(new THREE.BoxGeometry(0.34, 0.03, 0.2), dark, 0.05, 0.285, 0)
      add(new THREE.TorusGeometry(0.16, 0.018, 8, 24, Math.PI), rim, 0.05, 0.3, 0, 0, 0, 0)
      add(new THREE.CylinderGeometry(0.018, 0.018, 0.2, 8), rim, 0.2, 0.36, 0, 0, 0, Math.PI / 2 - 0.7)
      // Wings: front low and wide, rear high on two endplates.
      add(new THREE.BoxGeometry(0.1, 0.02, 0.9), body, 1.32, 0.09, 0)
      add(new THREE.BoxGeometry(0.06, 0.02, 0.9), dark, 1.22, 0.11, 0)
      add(new THREE.BoxGeometry(0.14, 0.03, 0.8), body, -1.0, 0.5, 0)
      add(new THREE.BoxGeometry(0.1, 0.02, 0.8), dark, -0.96, 0.44, 0)
      for (const s of [-1, 1]) add(new THREE.BoxGeometry(0.22, 0.24, 0.02), dark, -0.98, 0.4, s * 0.4)
      // Wheels, with a rim disc each.
      for (const [wx, wz] of [[0.85, 0.5], [0.85, -0.5], [-0.65, 0.52], [-0.65, -0.52]] as [number, number][]) {
        const rear = wx < 0
        add(new THREE.CylinderGeometry(rear ? 0.19 : 0.17, rear ? 0.19 : 0.17, rear ? 0.2 : 0.16, 24), tyre, wx, rear ? 0.19 : 0.17, wz, Math.PI / 2)
        add(new THREE.CylinderGeometry(0.1, 0.1, (rear ? 0.2 : 0.16) + 0.01, 16), rim, wx, rear ? 0.19 : 0.17, wz, Math.PI / 2)
      }
      // Suspension arms to the tub.
      for (const [wx, wz] of [[0.85, 0.3], [0.85, -0.3], [-0.65, 0.3], [-0.65, -0.3]] as [number, number][]) {
        add(new THREE.BoxGeometry(0.03, 0.02, 0.3), dark, wx, 0.2, wz)
      }
      return g
    }

    const plant = (x: number, z: number, scale = 1) => {
      const p = shadowed(new THREE.Mesh(new THREE.CylinderGeometry(0.2 * scale, 0.16 * scale, 0.4 * scale, 16), pot))
      p.position.set(x, 0.2 * scale, z)
      scene.add(p)
      for (let i = 0; i < 3; i++) {
        const ball = shadowed(new THREE.Mesh(new THREE.SphereGeometry((0.22 + i * 0.04) * scale, 16, 16), leaf))
        const a = (i / 3) * Math.PI * 2
        ball.position.set(x + Math.cos(a) * 0.1 * scale, (0.62 + i * 0.12) * scale, z + Math.sin(a) * 0.1 * scale)
        scene.add(ball)
      }
    }

    /* ---- clusters ---- */
    const clusters: Cluster[] = []
    for (const dept of departments) {
      const tone = toneColor(dept.tone)
      const rugMaterial = new THREE.MeshPhysicalMaterial({
        color: colors.elevated.clone().lerp(tone, dept.tone === 'advisory' ? 0.1 : 0.18),
        roughness: 0.85,
        emissive: tone.clone(),
        emissiveIntensity: 0,
      })
      const mat = new THREE.Mesh(rug(dept.w, dept.d), rugMaterial)
      mat.position.set(dept.x, 0.006, dept.z)
      mat.receiveShadow = true
      scene.add(mat)

      const facing: 1 | -1 = dept.z < 0 ? 1 : -1 // toward the aisle
      const centre = new THREE.Vector3(dept.x, 0, dept.z)
      const cluster: Cluster = {
        dept,
        centre,
        aisle: new THREE.Vector3(dept.x, 0, 0),
        rugMaterial,
        label: document.createElement('div'),
        figures: [],
        glow: 0,
        glowColor: tone.clone(),
      }

      if (dept.enclosed) {
        // Glass on all four sides with a door gap toward the aisle.
        const x0 = dept.x - dept.w / 2 - 0.25
        const x1 = dept.x + dept.w / 2 + 0.25
        const z0 = dept.z - dept.d / 2 - 0.25
        const z1 = dept.z + dept.d / 2 + 0.25
        const panel = (px: number, pz: number, w: number, d: number) => {
          const g = new THREE.Mesh(new THREE.BoxGeometry(w, GLASS_H, d), glass)
          g.position.set(px, GLASS_H / 2, pz)
          scene.add(g)
          const rail = new THREE.Mesh(new THREE.BoxGeometry(w + 0.02, 0.035, d + 0.02), frameMaterial)
          rail.position.set(px, GLASS_H + 0.017, pz)
          scene.add(rail)
          const sill = new THREE.Mesh(new THREE.BoxGeometry(w + 0.02, 0.05, d + 0.02), frameMaterial)
          sill.position.set(px, 0.025, pz)
          scene.add(sill)
        }
        const w = x1 - x0
        const d = z1 - z0
        const doorZ = facing === 1 ? z1 : z0
        const backZ = facing === 1 ? z0 : z1
        panel((x0 + x1) / 2, backZ, w, 0.04)
        panel(x0, (z0 + z1) / 2, 0.04, d)
        panel(x1, (z0 + z1) / 2, 0.04, d)
        const doorW = 1.0
        const side = (w - doorW) / 2
        panel(x0 + side / 2, doorZ, side, 0.04)
        panel(x1 - side / 2, doorZ, side, 0.04)
        for (const [px, pz] of [[x0, z0], [x1, z0], [x0, z1], [x1, z1]] as [number, number][]) {
          const postMesh = new THREE.Mesh(new THREE.BoxGeometry(0.07, GLASS_H + 0.05, 0.07), frameMaterial)
          postMesh.position.set(px, (GLASS_H + 0.05) / 2, pz)
          scene.add(postMesh)
        }
        plant(x0 + 0.45, backZ + facing * 0.45, 1.1)
      }

      const n = dept.occupants.length
      if (n > 0) {
        const material = () =>
          new THREE.MeshPhysicalMaterial({ color: tone, metalness: 0.08, roughness: 0.32, clearcoat: 1, clearcoatRoughness: 0.12, emissive: tone.clone(), emissiveIntensity: 0 })
        const seats: [number, number, 1 | -1][] = []
        if (dept.arrange === 'grid' && n > 2) {
          // Two rows facing each other across the rug.
          const cols = Math.ceil(n / 2)
          const spanX = (cols - 1) * 1.5
          for (let i = 0; i < n; i++) {
            const col = i % cols
            const row = Math.floor(i / cols)
            seats.push([dept.x - spanX / 2 + col * 1.5, dept.z + (row === 0 ? -0.55 : 0.55), row === 0 ? 1 : -1])
          }
        } else {
          const span = (n - 1) * 1.5
          for (let i = 0; i < n; i++) seats.push([dept.x - span / 2 + i * 1.5, dept.z, facing])
        }
        for (const [x, z, f] of seats) {
          const m = material()
          desk(x, z, f, m)
          cluster.figures.push({ material: m, glow: 0, glowColor: tone.clone() })
        }
      } else if (dept.furniture === 'racks') {
        // A server row: each feed its own cabinet, seven units high, every
        // unit with a drive bay and three activity lights. The lights flip at
        // their own rates so the row reads as running, not painted on.
        const names = dept.racks ?? ['MT5', 'Dukascopy', 'Binance']
        const pitch = 0.78
        // The cabinets face the outer edge, where the camera starts: the
        // aisle side is their back, which is how a server row is stood anyway.
        const front: 1 | -1 = -facing as 1 | -1
        names.forEach((name, i) => {
          const rx = dept.x - ((names.length - 1) * pitch) / 2 + i * pitch
          const rz = dept.z - front * 0.2
          const cabinet = shadowed(new THREE.Mesh(new THREE.BoxGeometry(0.62, 1.5, 0.62), equipment))
          cabinet.position.set(rx, 0.75, rz)
          scene.add(cabinet)
          // Door frame and a vented top.
          const frameBox = new THREE.Mesh(new THREE.BoxGeometry(0.66, 1.54, 0.03), frameMaterial)
          frameBox.position.set(rx, 0.77, rz + front * 0.31)
          scene.add(frameBox)
          for (let u = 0; u < 7; u++) {
            const uy = 0.2 + u * 0.19
            const unit = new THREE.Mesh(new THREE.BoxGeometry(0.54, 0.15, 0.04), rackUnit)
            unit.position.set(rx, uy, rz + front * 0.335)
            scene.add(unit)
            const bay = new THREE.Mesh(new THREE.BoxGeometry(0.22, 0.09, 0.01), deskLeg)
            bay.position.set(rx + 0.12, uy, rz + front * 0.36)
            scene.add(bay)
            for (let k = 0; k < 3; k++) {
              const warn = k === 2
              const on = warn ? colors.caution.clone() : colors.mint.clone()
              const material = new THREE.MeshBasicMaterial({ color: colors.background })
              const dot = new THREE.Mesh(ledGeometry, material)
              dot.position.set(rx - 0.19 + k * 0.09, uy + 0.03, rz + front * 0.365)
              scene.add(dot)
              const halo = haloFor(on, 0.11)
              halo.position.copy(dot.position).add(new THREE.Vector3(0, 0, front * 0.03))
              const lit = Math.random() < 0.5
              material.color.copy(lit ? on : colors.background.clone().lerp(on, 0.1))
              halo.material.opacity = lit ? 0.7 : 0
              leds.push({
                material,
                halo,
                on,
                off: colors.background.clone().lerp(on, 0.1),
                // Activity lights chatter; the amber one changes state about once a second.
                rate: warn ? 0.9 : 2 + Math.random() * 5,
                lit,
              })
            }
          }
          // Cable drop from the tray to each cabinet.
          const cable = new THREE.Mesh(new THREE.CylinderGeometry(0.012, 0.012, 0.5, 8), deskLeg)
          cable.position.set(rx + 0.2, 1.75, rz - front * 0.2)
          scene.add(cable)
          // The feed's name on the floor in front of its cabinet.
          const tag = document.createElement('div')
          tag.className =
            'pointer-events-none absolute -translate-x-1/2 whitespace-nowrap text-center text-[9px] font-mono text-muted-foreground [text-shadow:0_1px_3px_rgba(0,0,0,0.9)]'
          tag.textContent = name
          labelHost.appendChild(tag)
          rackTags.push({ label: tag, at: new THREE.Vector3(rx, 0.02, rz + front * 0.7) })
        })
        // A cable tray across the top of the row, and a switch on the middle
        // cabinet with a row of port lights that flicker faster than the rest.
        const trayW = (names.length - 1) * pitch + 0.7
        const tray = new THREE.Mesh(new THREE.BoxGeometry(trayW, 0.05, 0.16), frameMaterial)
        tray.position.set(dept.x, 2.0, dept.z - front * 0.4)
        scene.add(tray)
        for (const sx of [-1, 1]) {
          const leg = new THREE.Mesh(new THREE.BoxGeometry(0.04, 2.0, 0.04), frameMaterial)
          leg.position.set(dept.x + (sx * trayW) / 2, 1.0, dept.z - front * 0.4)
          scene.add(leg)
        }
        const sw = new THREE.Mesh(new THREE.BoxGeometry(0.5, 0.07, 0.4), rackUnit)
        sw.position.set(dept.x, 1.535, dept.z - front * 0.2)
        scene.add(sw)
        for (let k = 0; k < 8; k++) {
          const on = k % 4 === 3 ? colors.caution.clone() : colors.mint.clone()
          const material = new THREE.MeshBasicMaterial({ color: colors.background })
          const dot = new THREE.Mesh(ledGeometry, material)
          dot.position.set(dept.x - 0.21 + k * 0.06, 1.55, dept.z - front * 0.2 + front * 0.205)
          scene.add(dot)
          const halo = haloFor(on, 0.1)
          halo.position.copy(dot.position).add(new THREE.Vector3(0, 0, front * 0.03))
          const lit = Math.random() < 0.5
          material.color.copy(lit ? on : colors.background.clone().lerp(on, 0.1))
          halo.material.opacity = lit ? 0.7 : 0
          leds.push({ material, halo, on, off: colors.background.clone().lerp(on, 0.1), rate: 4 + Math.random() * 8, lit })
        }
      } else if (dept.furniture === 'engine') {
        const core = shadowed(new THREE.Mesh(new THREE.BoxGeometry(2.0, 0.85, 1.0), equipment))
        core.position.set(dept.x, 0.425, dept.z - facing * 0.15)
        scene.add(core)
        const strip = new THREE.Mesh(new THREE.BoxGeometry(1.8, 0.05, 0.03), new THREE.MeshBasicMaterial({ color: colors.primary }))
        strip.position.set(dept.x, 0.62, core.position.z + facing * 0.515)
        scene.add(strip)
        const vent = new THREE.Mesh(new THREE.BoxGeometry(1.6, 0.02, 0.6), frameMaterial)
        vent.position.set(dept.x, 0.86, core.position.z)
        scene.add(vent)
      } else if (dept.furniture === 'lounge') {
        // The lobby: the show car in the middle on a turning plinth under its
        // own light, a sofa group either side round a low table, the counter
        // at the west end facing the doors, plants where a lobby keeps them.
        const sofaMaterial = new THREE.MeshPhysicalMaterial({ color: colors.elevated.clone().lerp(colors.muted, 0.35), roughness: 0.85 })
        const sofa = (sx: number, sz: number, f: 1 | -1) => {
          const seat = shadowed(new THREE.Mesh(new THREE.BoxGeometry(1.7, 0.3, 0.6), sofaMaterial))
          seat.position.set(sx, 0.2, sz)
          scene.add(seat)
          const back = shadowed(new THREE.Mesh(new THREE.BoxGeometry(1.7, 0.35, 0.16), sofaMaterial))
          back.position.set(sx, 0.5, sz - f * 0.24)
          scene.add(back)
          for (const ax of [-1, 1]) {
            const arm = new THREE.Mesh(new THREE.BoxGeometry(0.14, 0.2, 0.6), sofaMaterial)
            arm.position.set(sx + ax * 0.8, 0.45, sz)
            scene.add(arm)
          }
        }
        const group = (gx: number) => {
          sofa(gx, dept.z - 1.0, 1)
          sofa(gx, dept.z + 1.0, -1)
          const table = shadowed(new THREE.Mesh(new THREE.CylinderGeometry(0.45, 0.45, 0.05, 32), deskTop))
          table.position.set(gx, 0.4, dept.z)
          scene.add(table)
          const tableLeg = new THREE.Mesh(new THREE.CylinderGeometry(0.05, 0.12, 0.38, 16), deskLeg)
          tableLeg.position.set(gx, 0.19, dept.z)
          scene.add(tableLeg)
        }
        group(dept.x - dept.w * 0.22)
        group(dept.x + dept.w * 0.22)
        // The counter, facing the doors on the south side.
        const counter = shadowed(new THREE.Mesh(new THREE.BoxGeometry(2.2, 1.0, 0.55), deskTop))
        counter.position.set(dept.x - dept.w / 2 + 1.6, 0.5, dept.z + 0.3)
        scene.add(counter)
        const counterTop = new THREE.Mesh(new THREE.BoxGeometry(2.3, 0.04, 0.65), frameMaterial)
        counterTop.position.set(counter.position.x, 1.02, counter.position.z)
        scene.add(counterTop)
        const counterScreen = new THREE.Mesh(new THREE.BoxGeometry(0.34, 0.22, 0.02), screenMaterial)
        counterScreen.position.set(counter.position.x + 0.5, 1.16, counter.position.z - 0.1)
        scene.add(counterScreen)
        chair(counter.position.x, counter.position.z - 0.7, 1)
        // The car, centre stage.
        const plinth = shadowed(new THREE.Mesh(new THREE.CylinderGeometry(1.55, 1.6, 0.12, 48), frameMaterial))
        plinth.position.set(dept.x, 0.06, dept.z)
        scene.add(plinth)
        const plinthTop = new THREE.Mesh(new THREE.CylinderGeometry(1.5, 1.5, 0.02, 48), equipment)
        plinthTop.position.set(dept.x, 0.13, dept.z)
        scene.add(plinthTop)
        const car = raceCar()
        car.position.set(dept.x, 0.14, dept.z)
        car.rotation.y = 0.5
        scene.add(car)
        showCar = car
        const carLight = new THREE.SpotLight(colors.foreground, 4.5, 8, 0.5, 0.7, 1.2)
        carLight.position.set(dept.x, 4.2, dept.z)
        carLight.target = car
        carLight.castShadow = true
        carLight.shadow.mapSize.set(1024, 1024)
        scene.add(carLight)
        const carHalo = haloFor(colors.caution, 1.6)
        carHalo.material.opacity = 0.18
        carHalo.position.set(dept.x, 0.5, dept.z)
        // A mat at the doors and plants at both ends.
        const mat = new THREE.Mesh(rug(4.0, 0.7, 0.2), new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.foreground, 0.12), roughness: 0.95 }))
        mat.position.set(dept.x, 0.009, dept.z + dept.d / 2 - 0.45)
        scene.add(mat)
        plant(dept.x - dept.w / 2 + 0.5, dept.z - dept.d / 2 + 0.5, 1.1)
        plant(dept.x + dept.w / 2 - 0.5, dept.z - dept.d / 2 + 0.5, 1.1)
        plant(dept.x + dept.w / 2 - 0.5, dept.z + dept.d / 2 - 0.5, 0.9)
        plant(dept.x - 2.6, dept.z + dept.d / 2 - 0.5, 0.8)
        plant(dept.x + 2.6, dept.z + dept.d / 2 - 0.5, 0.8)
      } else if (dept.furniture === 'meeting') {
        // A long table, three chairs a side, a screen on the back glass.
        const tableW = dept.w - 1.2
        const table = shadowed(new THREE.Mesh(new THREE.BoxGeometry(tableW, 0.05, 0.95), deskTop))
        table.position.set(dept.x, 0.72, dept.z)
        scene.add(table)
        for (const sx of [-1, 1]) {
          const legSlab = new THREE.Mesh(new THREE.BoxGeometry(0.06, 0.7, 0.8), deskLeg)
          legSlab.position.set(dept.x + (sx * (tableW - 0.4)) / 2, 0.35, dept.z)
          scene.add(legSlab)
        }
        for (let i = 0; i < 3; i++) {
          const cx = dept.x - (tableW - 0.9) / 2 + (i * (tableW - 0.9)) / 2
          chair(cx, dept.z - 0.85, 1)
          chair(cx, dept.z + 0.85, -1)
        }
        const screen = new THREE.Mesh(new THREE.BoxGeometry(1.2, 0.68, 0.04), screenMaterial)
        screen.position.set(dept.x, 1.1, dept.z - facing * (dept.d / 2 + 0.16))
        scene.add(screen)
        plant(dept.x + dept.w / 2 - 0.4, dept.z - facing * (dept.d / 2 - 0.4), 0.9)
      } else if (dept.furniture === 'shelves') {
        for (let i = 0; i < 2; i++) {
          const zz = dept.z - facing * (0.75 - i * 1.3)
          // An open shelf unit: two uprights and a back, the boards between.
          for (const sx of [-1, 1]) {
            const upright = shadowed(new THREE.Mesh(new THREE.BoxGeometry(0.05, 1.3, 0.34), equipment))
            upright.position.set(dept.x + (sx * (dept.w - 0.8)) / 2, 0.65, zz)
            scene.add(upright)
          }
          const back = new THREE.Mesh(new THREE.BoxGeometry(dept.w - 0.8, 1.3, 0.03), equipment)
          back.position.set(dept.x, 0.65, zz - facing * 0.16)
          scene.add(back)
          for (let k = 0; k < 3; k++) {
            const board = new THREE.Mesh(new THREE.BoxGeometry(dept.w - 0.9, 0.025, 0.3), frameMaterial)
            board.position.set(dept.x, 0.3 + k * 0.38, zz)
            scene.add(board)
            // Files on the shelf, in the tones of the desks that made them.
            for (let b = 0; b < 6; b++) {
              const file = new THREE.Mesh(
                new THREE.BoxGeometry(0.08, 0.26, 0.2),
                new THREE.MeshPhysicalMaterial({ color: (b % 3 === 0 ? colors.caution : b % 3 === 1 ? colors.mint : colors.foreground).clone().lerp(colors.elevated, 0.55), roughness: 0.8 }),
              )
              file.position.set(dept.x - (dept.w - 1.3) / 2 + b * ((dept.w - 1.3) / 5), 0.45 + k * 0.38, zz)
              scene.add(file)
            }
          }
        }
      }

      const label = cluster.label
      label.className =
        'pointer-events-none absolute -translate-x-1/2 whitespace-nowrap text-center transition-opacity duration-200 [text-shadow:0_1px_3px_rgba(0,0,0,0.9)]'
      const solo = n === 1 && dept.occupants[0].title === dept.title
      const who = n && !solo ? dept.occupants.map((o) => o.title).join(' · ') : dept.line
      const model = dept.occupants[0]?.model ?? ''
      label.innerHTML =
        `<div class="text-[11px] font-semibold tracking-tight" style="color:${tone.getStyle()}">${dept.title}</div>` +
        `<div class="text-[10px] text-muted-foreground">${who}</div>` +
        (model ? `<div class="text-[9px] font-mono text-muted-foreground/80">${model}</div>` : '')
      labelHost.appendChild(label)
      clusters.push(cluster)
    }

    // Plants along the edge, where an open plan keeps them.
    plant(FLOOR_W / 2 - 0.6, -FLOOR_D / 2 + 0.6, 0.9)
    plant(FLOOR_W / 2 - 0.6, 0.0, 0.9)
    plant(FLOOR_W / 2 - 1.6, 3.2, 1.0)
    plant(2.6, -FLOOR_D / 2 + 0.5, 0.9)

    const byId = (id: string) => clusters.find((c) => c.dept.id === id)
    const arbiter = clusters.find((c) => c.dept.tone === 'arbiter')
    const vetoes = clusters.filter((c) => c.dept.tone === 'veto')
    const advisors = clusters.filter((c) => c.dept.tone === 'advisory')
    const archive = byId('archive')
    const lab = byId('lab')
    const engine = byId('engine')

    /* ---- the file ---- */
    const fileGroup = new THREE.Group()
    const fileMaterial = new THREE.MeshPhysicalMaterial({ color: colors.foreground, roughness: 0.4, emissive: colors.mint.clone(), emissiveIntensity: 0.3 })
    const sheet = shadowed(new THREE.Mesh(new THREE.BoxGeometry(0.3, 0.02, 0.4), fileMaterial))
    fileGroup.add(sheet)
    const halo = new THREE.Sprite(new THREE.SpriteMaterial({ map: glow, color: colors.mint, transparent: true, opacity: 0.9, blending: THREE.AdditiveBlending, depthWrite: false }))
    halo.scale.setScalar(1.0)
    fileGroup.add(halo)
    fileGroup.visible = false
    scene.add(fileGroup)

    /* ---- rings and arcs ---- */
    const rings: Ring[] = []
    const ringGeometry = new THREE.RingGeometry(0.3, 0.42, 48)
    const ring = (at: THREE.Vector3, color: THREE.Color, veto: boolean) => {
      const mesh = new THREE.Mesh(
        ringGeometry,
        new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.8, blending: THREE.AdditiveBlending, depthWrite: false, side: THREE.DoubleSide }),
      )
      mesh.rotation.x = -Math.PI / 2
      mesh.position.copy(at).setY(0.02)
      scene.add(mesh)
      rings.push({ mesh, age: 0, life: veto ? 2.8 : 1.1, reach: veto ? 1.4 : 2.6 })
    }

    const arcs: Arc[] = []
    const TAIL_POINTS = 22
    const sendArc = (from: Cluster, to: Cluster, color: THREE.Color) => {
      const start = from.centre.clone().setY(1.4)
      const end = to.centre.clone().setY(1.4)
      const mid = start.clone().lerp(end, 0.5)
      mid.y += 1.0 + start.distanceTo(end) * 0.16
      const curve = new THREE.QuadraticBezierCurve3(start, mid, end)
      const head = new THREE.Sprite(new THREE.SpriteMaterial({ map: glow, color, transparent: true, blending: THREE.AdditiveBlending, depthWrite: false }))
      head.scale.setScalar(0.45)
      scene.add(head)
      const tailGeometry = new THREE.BufferGeometry()
      tailGeometry.setAttribute('position', new THREE.Float32BufferAttribute(new Float32Array(TAIL_POINTS * 3), 3))
      const tail = new THREE.Line(tailGeometry, new THREE.LineBasicMaterial({ color, transparent: true, opacity: 0.7, blending: THREE.AdditiveBlending, depthWrite: false }))
      scene.add(tail)
      arcs.push({ curve, t: 0, duration: 0.9 + start.distanceTo(end) * 0.08, head, tail, to })
      from.glow = 1
      from.glowColor.copy(color)
      for (const f of from.figures) {
        f.glow = 1
        f.glowColor.copy(color)
      }
    }

    /* ---- the route: cluster → aisle → aisle → cluster ---- */
    const Y = 0.95
    const pathBetween = (a: Cluster, b: Cluster): THREE.Vector3[] => [
      a.centre.clone().setY(Y),
      a.aisle.clone().setY(Y),
      b.aisle.clone().setY(Y),
      b.centre.clone().setY(Y),
    ]

    let route: Stop[] = []
    let stopIndex = 0
    let segment: THREE.Vector3[] = []
    let segmentT = 0
    let segmentLength = 0
    let dwell = 0
    let closed = false
    const SPEED = 2.4

    const planRoute = (): Stop[] => {
      if (!archive || !lab || !engine || !arbiter) return []
      return [
        { cluster: archive, dwell: 1.0, vetoChance: 0 },
        { cluster: lab, dwell: 1.6, vetoChance: 0 },
        { cluster: engine, dwell: 1.8, vetoChance: 0 },
        ...vetoes.map((cluster) => ({ cluster, dwell: 1.5, vetoChance: 0.22 })),
        { cluster: arbiter, dwell: 2.4, vetoChance: 0 },
        { cluster: archive, dwell: 1.2, vetoChance: 0 },
      ]
    }

    const startSegment = (from: Cluster, to: Cluster) => {
      segment = pathBetween(from, to)
      segmentT = 0
      segmentLength = 0
      for (let i = 1; i < segment.length; i++) segmentLength += segment[i].distanceTo(segment[i - 1])
    }

    const beginCycle = () => {
      route = planRoute()
      if (route.length === 0) return
      stopIndex = 0
      closed = false
      fileGroup.visible = true
      fileGroup.position.copy(route[0].cluster.centre).setY(Y)
      halo.material.color.copy(colors.mint)
      fileMaterial.emissive.copy(colors.mint)
      dwell = route[0].dwell
      segment = []
    }

    const timers: number[] = []
    const arrive = (stop: Stop) => {
      const c = stop.cluster
      const color = closed ? colors.caution : c.dept.tone === 'arbiter' ? colors.primary : colors.mint
      c.glow = 1
      c.glowColor.copy(color)
      for (const f of c.figures) {
        f.glow = 1
        f.glowColor.copy(color)
      }
      ring(c.centre, color, false)
      if (!closed && stop.vetoChance > 0 && Math.random() < stop.vetoChance) {
        // Stamped: the file turns amber and goes back to the archive, closed.
        closed = true
        ring(c.centre, colors.caution, true)
        c.glowColor.copy(colors.caution)
        for (const f of c.figures) f.glowColor.copy(colors.caution)
        halo.material.color.copy(colors.caution)
        fileMaterial.emissive.copy(colors.caution)
        if (archive) route = [...route.slice(0, stopIndex + 1), { cluster: archive, dwell: 1.4, vetoChance: 0 }]
      }
      if (c.dept.tone === 'arbiter') {
        advisors.forEach((adv, i) => {
          timers.push(window.setTimeout(() => sendArc(adv, c, colors.mint), 250 + i * 320))
        })
      }
    }

    const point = (t: number, out: THREE.Vector3) => {
      let remaining = t * segmentLength
      for (let i = 1; i < segment.length; i++) {
        const len = segment[i].distanceTo(segment[i - 1])
        if (remaining <= len || i === segment.length - 1) {
          return out.copy(segment[i - 1]).lerp(segment[i], len === 0 ? 1 : Math.min(1, remaining / len))
        }
        remaining -= len
      }
      return out.copy(segment[segment.length - 1])
    }

    /* ---- resize ---- */
    const resize = () => {
      const { width, height } = host.getBoundingClientRect()
      if (width === 0 || height === 0) return
      renderer.setSize(width, height, false)
      camera.aspect = width / height
      camera.updateProjectionMatrix()
    }
    resize()
    const observer = new ResizeObserver(resize)
    observer.observe(host)

    /* ---- frame loop ---- */
    const clock = new THREE.Clock()
    let frame = 0
    let idle = 0.8
    const projected = new THREE.Vector3()
    const scratch = new THREE.Vector3()

    const render = () => {
      const dt = Math.min(clock.getDelta(), 0.05)
      const now = clock.elapsedTime

      if (!reduceMotion) {
        if (!fileGroup.visible) {
          idle -= dt
          if (idle <= 0) beginCycle()
        } else if (segment.length === 0) {
          dwell -= dt
          if (dwell <= 0) {
            if (stopIndex + 1 < route.length) startSegment(route[stopIndex].cluster, route[stopIndex + 1].cluster)
            else {
              fileGroup.visible = false
              idle = 1.6
            }
          }
        } else {
          segmentT += (dt * SPEED) / Math.max(segmentLength, 0.001)
          if (segmentT >= 1) {
            stopIndex += 1
            segment = []
            fileGroup.position.copy(route[stopIndex].cluster.centre).setY(Y)
            dwell = route[stopIndex].dwell
            arrive(route[stopIndex])
          } else {
            point(segmentT, scratch)
            fileGroup.position.copy(scratch)
          }
        }
        fileGroup.position.y = Y + Math.sin(now * 3) * 0.04
        sheet.rotation.y = now * 0.8
        if (showCar) showCar.rotation.y += dt * 0.25

        for (let i = arcs.length - 1; i >= 0; i--) {
          const a = arcs[i]
          a.t += dt / a.duration
          const t = Math.min(a.t, 1)
          const eased = 1 - Math.pow(1 - t, 2.2)
          a.curve.getPointAt(eased, scratch)
          a.head.position.copy(scratch)
          const positions = a.tail.geometry.getAttribute('position') as THREE.BufferAttribute
          const tailStart = Math.max(0, eased - 0.3)
          for (let p = 0; p < TAIL_POINTS; p++) {
            a.curve.getPointAt(tailStart + ((eased - tailStart) * p) / (TAIL_POINTS - 1), scratch)
            positions.setXYZ(p, scratch.x, scratch.y, scratch.z)
          }
          positions.needsUpdate = true
          if (a.t >= 1) {
            a.to.glow = Math.max(a.to.glow, 0.7)
            for (const f of a.to.figures) f.glow = Math.max(f.glow, 0.7)
            scene.remove(a.head, a.tail)
            a.head.material.dispose()
            a.tail.geometry.dispose()
            a.tail.material.dispose()
            arcs.splice(i, 1)
          }
        }

        for (let i = rings.length - 1; i >= 0; i--) {
          const r = rings[i]
          r.age += dt
          const t = r.age / r.life
          const scale = 1 + (r.reach - 1) * (1 - Math.pow(1 - Math.min(t, 1), 2))
          r.mesh.scale.setScalar(scale)
          r.mesh.material.opacity = Math.max(0, 0.8 * (1 - t))
          if (r.age >= r.life) {
            scene.remove(r.mesh)
            r.mesh.material.dispose()
            rings.splice(i, 1)
          }
        }
      }

      // Activity lights: each flips with probability rate·dt per frame, so the
      // row blinks at data speed and never in step. They blink whatever the
      // motion setting: a few pixels changing colour on a rack is a status
      // light, not motion, and a dark row reads as a dead one.
      for (const l of leds) {
        if (Math.random() < l.rate * dt) {
          l.lit = !l.lit
          l.material.color.copy(l.lit ? l.on : l.off)
          l.halo.material.opacity = l.lit ? 0.7 : 0
        }
      }

      for (const c of clusters) {
        c.glow = Math.max(0, c.glow - dt * 0.9)
        c.rugMaterial.emissive.copy(c.glowColor)
        c.rugMaterial.emissiveIntensity = c.glow * 0.25
        for (const f of c.figures) {
          f.glow = Math.max(0, f.glow - dt * 1.2)
          f.material.emissive.copy(f.glowColor)
          f.material.emissiveIntensity = f.glow * 0.8
        }
      }

      controls.update()
      renderer.render(scene, camera)

      const { width, height } = renderer.domElement.getBoundingClientRect()
      for (const tag of rackTags) {
        projected.copy(tag.at).project(camera)
        tag.label.style.opacity = projected.z > 1 ? '0' : '1'
        tag.label.style.left = `${((projected.x + 1) / 2) * width}px`
        tag.label.style.top = `${((1 - projected.y) / 2) * height}px`
      }
      for (const c of clusters) {
        projected.copy(c.centre)
        projected.y = c.dept.enclosed ? GLASS_H + 0.5 : c.dept.furniture === 'racks' ? 2.55 : c.dept.furniture === 'lounge' ? 2.3 : 1.75
        projected.project(camera)
        c.label.style.opacity = projected.z > 1 ? '0' : '1'
        c.label.style.left = `${((projected.x + 1) / 2) * width}px`
        c.label.style.top = `${((1 - projected.y) / 2) * height}px`
      }
      frame = requestAnimationFrame(render)
    }
    frame = requestAnimationFrame(render)

    return () => {
      cancelAnimationFrame(frame)
      for (const t of timers) window.clearTimeout(t)
      observer.disconnect()
      controls.dispose()
      scene.environment?.dispose()
      scene.traverse((object) => {
        if (object instanceof THREE.Mesh || object instanceof THREE.Sprite || object instanceof THREE.Line) {
          object.geometry?.dispose()
          const material = object.material as THREE.Material | THREE.Material[]
          if (Array.isArray(material)) material.forEach((m) => m.dispose())
          else material?.dispose()
        }
      })
      ringGeometry.dispose()
      ledGeometry.dispose()
      glow.dispose()
      for (const c of clusters) c.label.remove()
      for (const tag of rackTags) tag.label.remove()
      renderer.dispose()
      renderer.domElement.remove()
    }
  }, [departments, animate])

  return (
    <div
      className={className}
      role="img"
      aria-label="The company as an open-plan floor: desks in clusters on either side of one aisle, and a single glass office in the corner for the arbiter. Data racks, the strategy desk, the engine bay and the three veto desks sit south of the aisle; the advisory desks and the archive shelves north of it. A lit file walks the aisle from the archive through the lab and the engine, past each veto desk — any of which can stamp it amber and send it back — into the glass office, and back to the archive."
    >
      <div ref={container} className="absolute inset-0 [&>canvas]:block [&>canvas]:h-full [&>canvas]:w-full" />
      <div ref={labels} className="pointer-events-none absolute inset-0" aria-hidden />
    </div>
  )
}
