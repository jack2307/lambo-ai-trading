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

import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { MeshoptDecoder } from 'three/examples/jsm/libs/meshopt_decoder.module.js'
import { loadModels, type ModelSet } from './officeAssets'

/** Kenney's Furniture Kit and Mini Characters (CC0), by file name. */
const FURNITURE = [
  'desk', 'chairDesk', 'computerScreen', 'computerKeyboard', 'computerMouse', 'loungeSofa', 'loungeChair', 'tableCoffee',
  'plantSmall1', 'plantSmall2', 'plantSmall3', 'pottedPlant', 'bookcaseOpen', 'cabinetTelevision', 'televisionModern',
  'lampRoundFloor', 'kitchenCoffeeMachine', 'kitchenFridgeSmall', 'kitchenCabinet', 'laptop', 'trashcan', 'coatRackStanding',
  'sideTableDrawers',
]
const CHARACTERS = ['character-male-a', 'character-female-a', 'character-male-b', 'character-female-b', 'character-male-c', 'character-female-c']
/**
 * Poly Haven (CC0), photoreal, packed at 1k. Preferred over Kenney wherever
 * a piece exists here; the map gives each piece's yaw so its front faces +z
 * like the rest of the floor.
 */
const PHOTOREAL: Record<string, { name: string; yaw: number }> = {
  desk: { name: 'metal_office_desk', yaw: 0 },
  chairDesk: { name: 'dining_chair_02', yaw: 0 },
  armChair: { name: 'modern_arm_chair_01', yaw: 0 },
  deskLamp: { name: 'desk_lamp_arm_01', yaw: 0 },
  laptop: { name: 'classic_laptop', yaw: 0 },
  notepads: { name: 'office_notepads', yaw: 0 },
  trashcan: { name: 'metal_trash_can', yaw: 0 },
  clock: { name: 'wall_clock', yaw: 0 },
  bookcaseOpen: { name: 'wooden_bookshelf_worn', yaw: 0 },
  shelves: { name: 'steel_frame_shelves_01', yaw: 0 },
  loungeSofa: { name: 'sofa_02', yaw: 0 },
  tableCoffee: { name: 'modern_coffee_table_01', yaw: 0 },
  tableRound: { name: 'coffee_table_round_01', yaw: 0 },
  coffeeCart: { name: 'CoffeeCart_01', yaw: 0 },
  television: { name: 'Television_01', yaw: 0 },
  chalkboard: { name: 'standing_chalkboard_01', yaw: 0 },
  drawers: { name: 'drawer_cabinet', yaw: 0 },
  table: { name: 'WoodenTable_01', yaw: 0 },
  plant1: { name: 'potted_plant_01', yaw: 0 },
  plant2: { name: 'potted_plant_02', yaw: 0 },
  plant3: { name: 'potted_plant_04', yaw: 0 },
  plant4: { name: 'calathea_orbifolia_01', yaw: 0 },
}
/** Kenney's models face +z. */
const KENNEY_YAW = 0
const CHAR_YAW = 0

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
  furniture?: 'racks' | 'engine' | 'shelves' | 'showcase' | 'meeting'
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
  /**
   * A glTF/GLB to stand on the lobby's plinth instead of the built-in car.
   * Loaded after the floor is up; the primitive car holds the spot until
   * then, so a slow network shows a car either way. Scaled to the plinth
   * from its bounding box, whatever units it was made in.
   */
  carModel?: string
  /** Where the Kenney models live (`<base>/<name>.glb`). Absent: primitives only. */
  modelsBase?: string
  /** Where the Poly Haven models live. Absent: Kenney and primitives only. */
  photorealBase?: string
  className?: string
}

const FLOOR_W = 24
/**
 * The floor, north to south: the showcase strip along the long north wall,
 * the north wing, the aisle, the south wing, the front edge. Walls stand on
 * the north and west edges.
 */
const FLOOR_Z0 = -6.4
const FLOOR_Z1 = 8.6
const FLOOR_D = FLOOR_Z1 - FLOOR_Z0
const FLOOR_ZC = (FLOOR_Z0 + FLOOR_Z1) / 2
const AISLE_Z = 4.3
const AISLE_X0 = -11.4
const AISLE_X1 = 11.4
const GLASS_H = 2.6

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
  /** The rug, which is what a click on the department hits. */
  rug: THREE.Mesh
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

export function OfficeFloor({ departments, animate, carModel, modelsBase = '/models/kenney', photorealBase = '/models/polyhaven', className }: Props) {
  const container = useRef<HTMLDivElement>(null)
  const labels = useRef<HTMLDivElement>(null)
  const [zoomed, setZoomed] = useState(false)
  const resetRef = useRef<() => void>(() => {})

  useEffect(() => {
    const host = container.current
    const labelHost = labels.current
    if (!host || !labelHost) return

    const reduceMotion = !animate
    // `?floor=lite` draws without shadows or the photoreal set: for weak
    // machines and for headless checks of the layout.
    const lite = new URLSearchParams(window.location.search).get('floor') === 'lite'

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
    renderer.toneMappingExposure = 1.0
    renderer.shadowMap.enabled = !lite
    renderer.shadowMap.type = THREE.PCFSoftShadowMap
    host.appendChild(renderer.domElement)

    // The floor is built once the models are in (or the deadline passes);
    // whatever did not arrive is drawn as a primitive instead.
    let teardown = () => {}
    let cancelled = false
    Promise.all([
      loadModels(modelsBase, [...FURNITURE, ...CHARACTERS]),
      loadModels(photorealBase, lite ? [] : Object.values(PHOTOREAL).map((p) => p.name), 12000),
    ]).then(([models, real]) => {
      if (!cancelled) build(models, real)
    })

    const build = (models: ModelSet, real: ModelSet) => {
    const mixers: THREE.AnimationMixer[] = []
    /**
     * People who move. Each has a home (a chair or a spot to stand), the
     * three clips, and a small state machine: rest at home, now and then walk
     * to somewhere on the floor, stand there a while, walk back, sit down.
     * Paths run along two lanes — the aisle and the walkway behind the north
     * wing — joined at the gaps between clusters, so nobody walks through a
     * desk.
     */
    interface Agent {
      group: THREE.Group
      mixer: THREE.AnimationMixer
      actions: Partial<Record<'sit' | 'idle' | 'walk', THREE.AnimationAction>>
      current: 'sit' | 'idle' | 'walk'
      home: { x: number; z: number; yaw: number; seated: boolean; y: number }
      state: 'home' | 'walking' | 'away'
      goal: 'home' | 'away'
      path: THREE.Vector3[]
      leg: number
      dwell: number
      /** How often this one leaves: desk workers rarely, roamers often. */
      restless: number
      /** A patrol: walk straight to this point and back, instead of anywhere. */
      beat?: THREE.Vector3
    }
    const agents: Agent[] = []
    const rand = (a: number, b: number) => a + Math.random() * (b - a)
    /** A photoreal piece by role, fitted and turned to face +z; null when the set lacks it. */
    const photo = (role: keyof typeof PHOTOREAL, fit: Parameters<ModelSet['instance']>[1]) => {
      const spec = PHOTOREAL[role]
      const inst = real.instance(spec.name, fit)
      if (inst) inst.group.rotation.y = spec.yaw
      return inst
    }
    /** Place a fitted instance; the yaw is added to the piece's own. */
    const place = (inst: { group: THREE.Group } | null, x: number, y: number, z: number, yaw = 0) => {
      if (!inst) return false
      inst.group.position.set(x, y, z)
      inst.group.rotation.y += yaw
      scene.add(inst.group)
      return true
    }
    const scene = new THREE.Scene()
    scene.fog = new THREE.Fog(colors.card, 34, 60)

    const pmrem = new THREE.PMREMGenerator(renderer)
    scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture
    scene.environmentIntensity = 0.5
    pmrem.dispose()

    const camera = new THREE.PerspectiveCamera(28, 1, 0.1, 120)
    camera.position.set(19, 23, 33)

    const controls = new OrbitControls(camera, renderer.domElement)
    // The wheel zooms only once the scene has been clicked (or with Ctrl
    // held), so scrolling the page over the panel still scrolls the page.
    controls.enableZoom = false
    controls.zoomToCursor = true
    controls.minDistance = 5
    controls.maxDistance = 52
    controls.enablePan = false
    controls.enableDamping = true
    controls.dampingFactor = 0.06
    controls.autoRotate = !reduceMotion
    controls.autoRotateSpeed = 0.12
    controls.minPolarAngle = Math.PI * 0.2
    controls.maxPolarAngle = Math.PI * 0.36
    controls.target.set(0.6, 0.2, 1.0)

    /* ---- light: a warm key as if from a window wall, a cool fill ---- */
    scene.add(new THREE.HemisphereLight(colors.foreground, colors.background, 0.45))
    const key = new THREE.DirectionalLight(colors.foreground.clone().lerp(colors.caution, 0.22), 1.7)
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
    // The interior: light oak underfoot, walnut on the walls, black steel
    // frames, white lacquer on the desks, grey carpet under the clusters.
    const timber = colors.caution.clone().lerp(colors.foreground, 0.5).lerp(colors.background, 0.14)
    const walnut = colors.caution.clone().lerp(colors.background, 0.7)
    const lacquer = colors.foreground.clone().lerp(colors.elevated, 0.06)
    const carpet = colors.elevated.clone().lerp(colors.muted, 0.32)
    const steelBlack = colors.background.clone().lerp(colors.foreground, 0.1)
    const slab = new THREE.Mesh(
      new THREE.BoxGeometry(FLOOR_W + 1.2, 0.22, FLOOR_D + 1.2),
      new THREE.MeshPhysicalMaterial({ color: colors.background, roughness: 0.5, clearcoat: 0.4, clearcoatRoughness: 0.4 }),
    )
    slab.position.set(0, -0.12, FLOOR_ZC)
    slab.receiveShadow = true
    scene.add(slab)
    const floor = new THREE.Mesh(
      new THREE.PlaneGeometry(FLOOR_W, FLOOR_D),
      new THREE.MeshPhysicalMaterial({ color: timber, roughness: 0.42, clearcoat: 0.9, clearcoatRoughness: 0.28 }),
    )
    floor.rotation.x = -Math.PI / 2
    floor.position.z = FLOOR_ZC
    floor.receiveShadow = true
    scene.add(floor)
    // Plank seams, so the field reads as timber and not paint.
    const seam = new THREE.MeshBasicMaterial({ color: timber.clone().lerp(colors.background, 0.28) })
    for (let z = FLOOR_Z0 + 0.6; z < FLOOR_Z1; z += 0.6) {
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
    aisle.position.set((AISLE_X0 + AISLE_X1) / 2, 0.004, AISLE_Z)
    aisle.receiveShadow = true
    scene.add(aisle)

    /* ---- shared materials ---- */
    const glass = new THREE.MeshPhysicalMaterial({
      color: colors.foreground.clone().lerp(colors.caution, 0.08),
      transparent: true,
      opacity: 0.09,
      roughness: 0.05,
      clearcoat: 1,
      side: THREE.DoubleSide,
      depthWrite: false,
    })
    const frameMaterial = new THREE.MeshPhysicalMaterial({ color: steelBlack, roughness: 0.35, metalness: 0.6 })
    const deskTop = new THREE.MeshPhysicalMaterial({ color: lacquer, roughness: 0.22, clearcoat: 1, clearcoatRoughness: 0.08 })
    const deskLeg = new THREE.MeshPhysicalMaterial({ color: steelBlack, roughness: 0.4, metalness: 0.6 })
    const chairMaterial = new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.foreground, 0.1), roughness: 0.6 })
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
    const fans: THREE.Mesh[] = []
    const rugMeshes: THREE.Mesh[] = []
    let disposed = false
    /** Geometry, materials and every texture a material holds. */
    const disposeObject = (root: THREE.Object3D) => {
      root.traverse((object) => {
        if (object instanceof THREE.Mesh || object instanceof THREE.Sprite || object instanceof THREE.Line) {
          object.geometry?.dispose()
          const materials = Array.isArray(object.material) ? object.material : [object.material]
          for (const m of materials as THREE.Material[]) {
            if (!m) continue
            for (const value of Object.values(m)) if (value instanceof THREE.Texture) value.dispose()
            m.dispose()
          }
        }
      })
    }
    const leaf = new THREE.MeshPhysicalMaterial({ color: colors.mint.clone().lerp(colors.background, 0.55), roughness: 0.7 })
    const leafDark = new THREE.MeshPhysicalMaterial({ color: colors.mint.clone().lerp(colors.background, 0.7), roughness: 0.75 })
    const pot = new THREE.MeshPhysicalMaterial({ color: colors.elevated.clone().lerp(colors.caution, 0.12), roughness: 0.6 })
    const paper = new THREE.MeshPhysicalMaterial({ color: colors.foreground.clone().lerp(colors.elevated, 0.15), roughness: 0.6 })
    const steel = new THREE.MeshPhysicalMaterial({ color: colors.foreground.clone().lerp(colors.muted, 0.5), roughness: 0.25, metalness: 0.85 })
    const lampShade = new THREE.MeshPhysicalMaterial({ color: colors.foreground, roughness: 0.6, emissive: colors.caution.clone().lerp(colors.foreground, 0.5), emissiveIntensity: 0.55 })
    // People: skin and hair are the amber token pulled toward white and
    // toward black; the shirt is the department's tone.
    const skin = new THREE.MeshPhysicalMaterial({ color: colors.caution.clone().lerp(colors.foreground, 0.58), roughness: 0.65 })
    const hairMaterials = [
      new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.caution, 0.18), roughness: 0.8 }),
      new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.foreground, 0.3), roughness: 0.8 }),
      new THREE.MeshPhysicalMaterial({ color: colors.caution.clone().lerp(colors.background, 0.5), roughness: 0.8 }),
    ]
    const trousers = new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.foreground, 0.14), roughness: 0.85 })
    const shoe = new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.foreground, 0.06), roughness: 0.5 })
    const people: { head: THREE.Object3D; hands: THREE.Object3D[]; phase: number; typing: boolean }[] = []
    let personCount = 0
    const bookMaterials = [colors.caution, colors.mint, colors.foreground, colors.muted].map(
      (c) => new THREE.MeshPhysicalMaterial({ color: c.clone().lerp(colors.elevated, 0.5), roughness: 0.85 }),
    )

    const shadowed = (mesh: THREE.Mesh) => {
      mesh.castShadow = true
      mesh.receiveShadow = true
      return mesh
    }

    /**
     * A person. Built facing +z inside a group, then turned by `yaw`. Seated
     * poses take the seat height so the same figure sits a chair or a sofa;
     * `armsOnDesk` puts the forearms out at desk height, otherwise on the lap.
     */
    const person = (
      x: number,
      z: number,
      yaw: number,
      shirt: THREE.Material,
      pose: { kind: 'seated'; seat: number; armsOnDesk: boolean } | { kind: 'standing' },
    ) => {
      if (models.has(CHARACTERS[0])) {
        // A Kenney character: sits or stands from its own clips.
        const idx = personCount++
        const inst = models.instance(CHARACTERS[idx % CHARACTERS.length], { height: 1.08 })
        if (inst) {
          const seated = pose.kind === 'seated'
          const mixer = new THREE.AnimationMixer(inst.group)
          const actions: Agent['actions'] = {}
          for (const name of ['sit', 'idle', 'walk'] as const) {
            const clip = THREE.AnimationClip.findByName(inst.animations, name)
            if (!clip) continue
            const action = mixer.clipAction(clip)
            action.enabled = true
            action.setEffectiveWeight(name === (seated ? 'sit' : 'idle') ? 1 : 0)
            action.play()
            actions[name] = action
          }
          mixer.update(0.016)
          mixers.push(mixer)
          // Pose first, then stand the feet on the floor: the sit clip folds
          // the legs, and the chair was scaled with the same system, so the
          // hips land on the seat.
          inst.group.updateMatrixWorld(true)
          const posed = new THREE.Box3().setFromObject(inst.group)
          // The sit clip keeps the feet at the floor; lift the figure so the
          // hips land on the seat, a little over a third of the seat height.
          const y = seated ? -posed.min.y + pose.seat * 0.36 : 0
          inst.group.position.set(x, y, z)
          inst.group.rotation.y = yaw + CHAR_YAW
          scene.add(inst.group)
          agents.push({
            group: inst.group,
            mixer,
            actions,
            current: seated ? 'sit' : 'idle',
            home: { x, z, yaw: yaw + CHAR_YAW, seated, y },
            state: 'home',
            goal: 'home',
            path: [],
            leg: 0,
            // Desk workers work: a long sit before the first errand, and
            // most errand rolls come up empty. Standing people move more.
            dwell: seated ? rand(40, 150) : rand(6, 30),
            restless: seated ? 0.12 : 0.45,
          })
          return inst.group
        }
      }
      const g = new THREE.Group()
      const n = personCount++
      const hair = hairMaterials[n % hairMaterials.length]
      const tall = 0.94 + ((n * 7) % 5) * 0.03
      const part = (geometry: THREE.BufferGeometry, material: THREE.Material, px: number, py: number, pz: number, rx = 0, ry = 0, rz = 0) => {
        const m = shadowed(new THREE.Mesh(geometry, material))
        m.position.set(px, py, pz)
        m.rotation.set(rx, ry, rz)
        g.add(m)
        return m
      }
      const seat = pose.kind === 'seated' ? pose.seat : 0.86
      // Legs.
      if (pose.kind === 'seated') {
        for (const sx of [-0.09, 0.09]) {
          part(new THREE.CapsuleGeometry(0.07, 0.26, 4, 12), trousers, sx, seat - 0.05, 0.17, Math.PI / 2)
          const shin = seat - 0.14
          part(new THREE.CapsuleGeometry(0.06, Math.max(0.1, shin - 0.12), 4, 12), trousers, sx, shin / 2 + 0.06, 0.33)
          part(new THREE.BoxGeometry(0.1, 0.06, 0.2), shoe, sx, 0.03, 0.38)
        }
      } else {
        for (const sx of [-0.09, 0.09]) {
          part(new THREE.CapsuleGeometry(0.07, 0.3, 4, 12), trousers, sx, 0.62, 0)
          part(new THREE.CapsuleGeometry(0.06, 0.28, 4, 12), trousers, sx, 0.26, 0.01)
          part(new THREE.BoxGeometry(0.1, 0.06, 0.22), shoe, sx, 0.03, 0.05)
        }
      }
      // Torso, a little wider at the shoulders.
      const torso = part(new THREE.CapsuleGeometry(0.15, 0.3, 6, 16), shirt, 0, seat + 0.3, 0)
      torso.scale.set(1.25, 1, 0.85)
      part(new THREE.CylinderGeometry(0.045, 0.05, 0.1, 10), skin, 0, seat + 0.56, 0)
      // Head and hair.
      const head = part(new THREE.SphereGeometry(0.125, 20, 20), skin, 0, seat + 0.7, 0)
      const cap = new THREE.Mesh(new THREE.SphereGeometry(0.132, 20, 20, 0, Math.PI * 2, 0, Math.PI * 0.55), hair)
      cap.position.set(0, 0.015, -0.02)
      head.add(cap)
      const nose = new THREE.Mesh(new THREE.ConeGeometry(0.018, 0.04, 8), skin)
      nose.position.set(0, -0.01, 0.125)
      nose.rotation.x = Math.PI / 2
      head.add(nose)
      // Arms.
      const hands: THREE.Object3D[] = []
      for (const sx of [-1, 1]) {
        if (pose.kind === 'seated' && pose.armsOnDesk) {
          part(new THREE.CapsuleGeometry(0.045, 0.2, 4, 10), shirt, sx * 0.2, seat + 0.38, 0.1, 0.95)
          part(new THREE.CapsuleGeometry(0.04, 0.22, 4, 10), skin, sx * 0.17, seat + 0.29, 0.33, Math.PI / 2)
          hands.push(part(new THREE.SphereGeometry(0.045, 10, 10), skin, sx * 0.16, seat + 0.29, 0.47))
        } else if (pose.kind === 'seated') {
          part(new THREE.CapsuleGeometry(0.045, 0.2, 4, 10), shirt, sx * 0.2, seat + 0.36, 0.03, 0.4)
          part(new THREE.CapsuleGeometry(0.04, 0.2, 4, 10), skin, sx * 0.14, seat + 0.1, 0.2, Math.PI / 2)
          hands.push(part(new THREE.SphereGeometry(0.045, 10, 10), skin, sx * 0.13, seat + 0.1, 0.32))
        } else {
          part(new THREE.CapsuleGeometry(0.045, 0.22, 4, 10), shirt, sx * 0.21, seat + 0.34, 0, 0, 0, sx * 0.12)
          part(new THREE.CapsuleGeometry(0.04, 0.2, 4, 10), skin, sx * 0.24, seat + 0.06, 0.02, -0.25)
          hands.push(part(new THREE.SphereGeometry(0.045, 10, 10), skin, sx * 0.25, seat - 0.08, 0.06))
        }
      }
      g.position.set(x, 0, z)
      g.rotation.y = yaw
      g.scale.setScalar(tall)
      scene.add(g)
      people.push({ head, hands, phase: n * 1.7, typing: pose.kind === 'seated' && pose.armsOnDesk })
      return g
    }

    /** A chair at (x, z) whose back is on the −facing side. */
    const chair = (x: number, z: number, facing: 1 | -1) => {
      const model = models.instance('chairDesk', { height: 0.9 }, steelBlack.clone().lerp(colors.foreground, 0.06))
      if (model) {
        model.group.position.set(x, 0, z)
        model.group.rotation.y = (facing === 1 ? 0 : Math.PI) + KENNEY_YAW
        scene.add(model.group)
        return
      }
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
      {
        // White lacquer top on slim black legs, a white pedestal on one side,
        // a black monitor; the laptop, notepads and the bin are real when here.
        const yaw = facing === 1 ? 0 : Math.PI
        const top = 0.74
        const tabletop = shadowed(new THREE.Mesh(new THREE.BoxGeometry(1.5, 0.04, 0.72), deskTop))
        tabletop.position.set(x, top - 0.02, z)
        scene.add(tabletop)
        const pedestal = shadowed(new THREE.Mesh(new THREE.BoxGeometry(0.42, 0.62, 0.62), deskTop))
        pedestal.position.set(x + 0.5, 0.31, z)
        scene.add(pedestal)
        for (let k = 0; k < 3; k++) {
          const line = new THREE.Mesh(new THREE.BoxGeometry(0.4, 0.006, 0.01), deskLeg)
          line.position.set(x + 0.5, 0.15 + k * 0.2, z - facing * 0.31)
          scene.add(line)
        }
        for (const dz of [-0.3, 0.3]) {
          const leg = new THREE.Mesh(new THREE.BoxGeometry(0.03, top - 0.04, 0.03), deskLeg)
          leg.position.set(x - 0.7, (top - 0.04) / 2, z + dz)
          scene.add(leg)
        }
        const beam = new THREE.Mesh(new THREE.BoxGeometry(0.03, 0.03, 0.62), deskLeg)
        beam.position.set(x - 0.7, top - 0.06, z)
        scene.add(beam)
        const screen = new THREE.Mesh(new THREE.BoxGeometry(0.5, 0.3, 0.02), screenMaterial)
        screen.position.set(x - 0.1, top + 0.27, z + facing * 0.18)
        scene.add(screen)
        const bezel = new THREE.Mesh(new THREE.BoxGeometry(0.54, 0.34, 0.015), deskLeg)
        bezel.position.set(x - 0.1, top + 0.27, z + facing * 0.19)
        scene.add(bezel)
        const stand = new THREE.Mesh(new THREE.BoxGeometry(0.04, 0.14, 0.04), deskLeg)
        stand.position.set(x - 0.1, top + 0.07, z + facing * 0.18)
        scene.add(stand)
        const foot = new THREE.Mesh(new THREE.BoxGeometry(0.22, 0.012, 0.16), deskLeg)
        foot.position.set(x - 0.1, top + 0.006, z + facing * 0.18)
        scene.add(foot)
        place(photo('laptop', { width: 0.32 }), x + 0.42, top, z + facing * 0.05, yaw)
        place(photo('notepads', { width: 0.2 }), x - 0.5, top, z - facing * 0.1, yaw + 0.3)
        const keyboard = new THREE.Mesh(new THREE.BoxGeometry(0.36, 0.014, 0.12), deskLeg)
        keyboard.position.set(x - 0.1, top + 0.007, z - facing * 0.1)
        scene.add(keyboard)
        place(photo('trashcan', { height: 0.32 }), x - 0.95, 0, z - facing * 0.2)
        // The chair's centre includes its back; the sitter's hips go on the
        // seat, forward of that, so the body does not sink into the backrest.
        const cz = z - facing * 0.74
        chair(x - 0.1, cz - facing * 0.04, facing)
        person(x - 0.1, cz + facing * 0.16, facing === 1 ? 0 : Math.PI, material, { kind: 'seated', seat: 0.46, armsOnDesk: true })
        return
      }
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

    let plantCount = 0
    const plant = (x: number, z: number, scale = 1) => {
      const realKinds = ['plant1', 'plant2', 'plant3', 'plant4'] as const
      const realPlant = photo(realKinds[plantCount % realKinds.length], { height: 0.9 * scale })
      if (realPlant) {
        plantCount += 1
        place(realPlant, x, 0, z, (plantCount * 1.9) % (Math.PI * 2))
        return
      }
      const kinds = ['plantSmall1', 'plantSmall2', 'plantSmall3', 'pottedPlant']
      const model = models.instance(kinds[plantCount++ % kinds.length], { height: 0.95 * scale })
      if (model) {
        model.group.position.set(x, 0, z)
        model.group.rotation.y = (plantCount * 1.9) % (Math.PI * 2)
        scene.add(model.group)
        return
      }
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

    /** A tall floor plant: the big potted one when it is here, else a palm of cones. */
    const palm = (x: number, z: number, scale = 1) => {
      const tall = photo('plant1', { height: 1.75 * scale })
      if (tall) {
        place(tall, x, 0, z, (x * 3 + z) % (Math.PI * 2))
        return
      }
      const p = shadowed(new THREE.Mesh(new THREE.CylinderGeometry(0.26 * scale, 0.22 * scale, 0.5 * scale, 16), pot))
      p.position.set(x, 0.25 * scale, z)
      scene.add(p)
      const trunk = shadowed(new THREE.Mesh(new THREE.CylinderGeometry(0.05 * scale, 0.08 * scale, 1.5 * scale, 10), new THREE.MeshPhysicalMaterial({ color: colors.elevated.clone().lerp(colors.caution, 0.2), roughness: 0.9 })))
      trunk.position.set(x, 1.2 * scale, z)
      scene.add(trunk)
      for (let i = 0; i < 6; i++) {
        const a = (i / 6) * Math.PI * 2
        const frond = shadowed(new THREE.Mesh(new THREE.ConeGeometry(0.1 * scale, 0.9 * scale, 6), i % 2 ? leaf : leafDark))
        frond.position.set(x + Math.cos(a) * 0.32 * scale, 1.95 * scale, z + Math.sin(a) * 0.32 * scale)
        frond.rotation.set(Math.sin(a) * 1.15, 0, -Math.cos(a) * 1.15)
        scene.add(frond)
      }
    }

    /** A whiteboard on a stand, its face toward +facing z, with a few lines on it. */
    const whiteboard = (x: number, z: number, facing: 1 | -1) => {
      if (place(photo('chalkboard', { height: 1.7 }), x, 0, z, facing === 1 ? 0 : Math.PI)) return
      const board = shadowed(new THREE.Mesh(new THREE.BoxGeometry(1.4, 0.9, 0.04), paper))
      board.position.set(x, 1.15, z)
      scene.add(board)
      for (const dx of [-0.6, 0.6]) {
        const leg = new THREE.Mesh(new THREE.BoxGeometry(0.04, 1.6, 0.04), frameMaterial)
        leg.position.set(x + dx, 0.8, z)
        scene.add(leg)
      }
      for (let i = 0; i < 4; i++) {
        const w = 0.5 + ((i * 7) % 5) * 0.14
        const line = new THREE.Mesh(new THREE.PlaneGeometry(w, 0.03), new THREE.MeshBasicMaterial({ color: i === 1 ? colors.caution : colors.mint }))
        line.position.set(x - 0.6 + w / 2, 1.45 - i * 0.18, z + facing * 0.025)
        if (facing === -1) line.rotation.y = Math.PI
        scene.add(line)
      }
    }

    /** A floor lamp: post, shade, and a warm glow. */
    const floorLamp = (x: number, z: number) => {
      const model = models.instance('lampRoundFloor', { height: 1.7 })
      if (model) {
        model.group.position.set(x, 0, z)
        scene.add(model.group)
        const g = haloFor(colors.caution.clone().lerp(colors.foreground, 0.5), 0.9)
        g.material.opacity = 0.35
        g.position.set(x, 1.55, z)
        return
      }
      const base = new THREE.Mesh(new THREE.CylinderGeometry(0.16, 0.16, 0.03, 20), steel)
      base.position.set(x, 0.015, z)
      scene.add(base)
      const post = new THREE.Mesh(new THREE.CylinderGeometry(0.02, 0.02, 1.6, 8), steel)
      post.position.set(x, 0.8, z)
      scene.add(post)
      const shade = shadowed(new THREE.Mesh(new THREE.CylinderGeometry(0.16, 0.22, 0.28, 20, 1, true), lampShade))
      shade.position.set(x, 1.7, z)
      scene.add(shade)
      const g = haloFor(colors.caution.clone().lerp(colors.foreground, 0.5), 0.9)
      g.material.opacity = 0.35
      g.position.set(x, 1.62, z)
    }

    /** A desk lamp with an amber shade. */
    const deskLamp = (x: number, y: number, z: number) => {
      const lamp = photo('deskLamp', { height: 0.45 })
      if (lamp) {
        place(lamp, x, y, z, Math.PI * 0.75)
        const g = haloFor(colors.caution.clone().lerp(colors.foreground, 0.5), 0.4)
        g.material.opacity = 0.45
        g.position.set(x - 0.05, y + 0.32, z)
        return
      }
      const arm = new THREE.Mesh(new THREE.CylinderGeometry(0.012, 0.012, 0.36, 8), steel)
      arm.position.set(x, y + 0.18, z)
      arm.rotation.z = 0.35
      scene.add(arm)
      const shade = new THREE.Mesh(new THREE.ConeGeometry(0.09, 0.1, 16, 1, true), lampShade)
      shade.position.set(x - 0.07, y + 0.36, z)
      scene.add(shade)
      const g = haloFor(colors.caution.clone().lerp(colors.foreground, 0.5), 0.4)
      g.material.opacity = 0.45
      g.position.set(x - 0.07, y + 0.3, z)
    }

    /** A filing cabinet. */
    const cabinet = (x: number, z: number, drawersFacing: 1 | -1) => {
      if (place(photo('drawers', { height: 0.85 }), x, 0, z, drawersFacing === 1 ? 0 : Math.PI)) return
      const model = models.instance('sideTableDrawers', { height: 0.8 })
      if (model) {
        model.group.position.set(x, 0, z)
        model.group.rotation.y = (drawersFacing === 1 ? 0 : Math.PI) + KENNEY_YAW
        scene.add(model.group)
        return
      }
      const body = shadowed(new THREE.Mesh(new THREE.BoxGeometry(0.5, 1.1, 0.55), equipment))
      body.position.set(x, 0.55, z)
      scene.add(body)
      for (let i = 0; i < 3; i++) {
        const handle = new THREE.Mesh(new THREE.BoxGeometry(0.16, 0.02, 0.02), steel)
        handle.position.set(x, 0.25 + i * 0.33, z + drawersFacing * 0.285)
        scene.add(handle)
      }
    }

    /** A bookcase against a back wall, spines toward +facing z. */
    const bookcase = (x: number, z: number, width: number, facing: 1 | -1) => {
      if (real.has(PHOTOREAL.bookcaseOpen.name)) {
        const unitW = 1.1
        const n = Math.max(1, Math.floor(width / unitW))
        for (let i = 0; i < n; i++) {
          if (!place(photo('bookcaseOpen', { width: unitW }), x - ((n - 1) * unitW) / 2 + i * unitW, 0, z, facing === 1 ? 0 : Math.PI)) break
        }
        return
      }
      if (models.has('bookcaseOpen')) {
        // As many units as the wall takes, side by side.
        const unitW = 1.0
        const n = Math.max(1, Math.floor(width / unitW))
        for (let i = 0; i < n; i++) {
          const inst = models.instance('bookcaseOpen', { width: unitW })
          if (!inst) break
          inst.group.position.set(x - ((n - 1) * unitW) / 2 + i * unitW, 0, z)
          inst.group.rotation.y = (facing === 1 ? 0 : Math.PI) + KENNEY_YAW
          scene.add(inst.group)
        }
        return
      }
      const back = shadowed(new THREE.Mesh(new THREE.BoxGeometry(width, 1.7, 0.04), equipment))
      back.position.set(x, 0.85, z - facing * 0.15)
      scene.add(back)
      for (const sx of [-1, 1]) {
        const side = new THREE.Mesh(new THREE.BoxGeometry(0.04, 1.7, 0.32), equipment)
        side.position.set(x + (sx * width) / 2, 0.85, z)
        scene.add(side)
      }
      for (let k = 0; k < 4; k++) {
        const board = new THREE.Mesh(new THREE.BoxGeometry(width, 0.025, 0.3), frameMaterial)
        board.position.set(x, 0.2 + k * 0.42, z)
        scene.add(board)
        let bx = x - width / 2 + 0.08
        let i = 0
        while (bx < x + width / 2 - 0.1) {
          const w = 0.05 + ((i * 3) % 4) * 0.015
          const h = 0.24 + ((i * 5) % 3) * 0.04
          const book = new THREE.Mesh(new THREE.BoxGeometry(w, h, 0.2), bookMaterials[(i + k) % bookMaterials.length])
          book.position.set(bx + w / 2, 0.2 + k * 0.42 + h / 2 + 0.013, z)
          scene.add(book)
          bx += w + 0.012
          i += 1
        }
      }
    }

    /** A coffee station: a counter, an espresso machine with a lit light, cups, a kettle. */
    const coffeeStation = (x: number, z: number, facing: 1 | -1) => {
      if (models.has('kitchenCabinet')) {
        const yaw = (facing === 1 ? 0 : Math.PI) + KENNEY_YAW
        let top = 0.9
        for (const dx of [-0.5, 0.5]) {
          const cab = models.instance('kitchenCabinet', { width: 1.0 })
          if (!cab) break
          cab.group.position.set(x + dx, 0, z)
          cab.group.rotation.y = yaw
          scene.add(cab.group)
          top = cab.size.y
        }
        const machine = models.instance('kitchenCoffeeMachine', { height: 0.42 })
        if (machine) {
          machine.group.position.set(x - 0.45, top, z)
          machine.group.rotation.y = yaw
          scene.add(machine.group)
        }
        for (let i = 0; i < 4; i++) {
          const cup = new THREE.Mesh(new THREE.CylinderGeometry(0.04, 0.03, 0.08, 12), paper)
          cup.position.set(x + 0.15 + i * 0.13, top + 0.04, z - facing * 0.12)
          scene.add(cup)
        }
        const fridge = models.instance('kitchenFridgeSmall', { height: 1.25 })
        if (fridge) {
          fridge.group.position.set(x + 1.35, 0, z)
          fridge.group.rotation.y = yaw
          scene.add(fridge.group)
        }
        const light = new THREE.Mesh(ledGeometry, new THREE.MeshBasicMaterial({ color: colors.mint }))
        light.position.set(x - 0.3, top + 0.3, z + facing * 0.2)
        scene.add(light)
        const lightHalo = haloFor(colors.mint, 0.12)
        lightHalo.material.opacity = 0.7
        lightHalo.position.copy(light.position).add(new THREE.Vector3(0, 0, facing * 0.03))
        return
      }
      const counter = shadowed(new THREE.Mesh(new THREE.BoxGeometry(1.8, 0.9, 0.6), deskTop))
      counter.position.set(x, 0.45, z)
      scene.add(counter)
      const top = new THREE.Mesh(new THREE.BoxGeometry(1.9, 0.04, 0.7), frameMaterial)
      top.position.set(x, 0.92, z)
      scene.add(top)
      // The machine.
      const body = shadowed(new THREE.Mesh(new THREE.BoxGeometry(0.42, 0.4, 0.4), steel))
      body.position.set(x - 0.5, 1.14, z)
      scene.add(body)
      const group = new THREE.Mesh(new THREE.CylinderGeometry(0.05, 0.05, 0.08, 12), steel)
      group.position.set(x - 0.5, 0.98, z + facing * 0.16)
      scene.add(group)
      const tray = new THREE.Mesh(new THREE.BoxGeometry(0.3, 0.02, 0.14), frameMaterial)
      tray.position.set(x - 0.5, 0.95, z + facing * 0.16)
      scene.add(tray)
      const light = new THREE.Mesh(ledGeometry, new THREE.MeshBasicMaterial({ color: colors.mint }))
      light.position.set(x - 0.38, 1.26, z + facing * 0.205)
      scene.add(light)
      const lightHalo = haloFor(colors.mint, 0.12)
      lightHalo.material.opacity = 0.7
      lightHalo.position.copy(light.position).add(new THREE.Vector3(0, 0, facing * 0.03))
      // Cups in a row and a kettle.
      for (let i = 0; i < 4; i++) {
        const cup = new THREE.Mesh(new THREE.CylinderGeometry(0.04, 0.03, 0.08, 12), paper)
        cup.position.set(x + 0.05 + i * 0.12, 0.98, z - facing * 0.12)
        scene.add(cup)
      }
      const kettle = shadowed(new THREE.Mesh(new THREE.CylinderGeometry(0.08, 0.1, 0.2, 16), steel))
      kettle.position.set(x + 0.65, 1.04, z)
      scene.add(kettle)
      const jar = new THREE.Mesh(new THREE.CylinderGeometry(0.06, 0.06, 0.16, 16), glass)
      jar.position.set(x + 0.35, 1.02, z + facing * 0.15)
      scene.add(jar)
    }

    /** A water cooler. */
    const waterCooler = (x: number, z: number) => {
      const body = shadowed(new THREE.Mesh(new THREE.BoxGeometry(0.34, 0.95, 0.34), paper))
      body.position.set(x, 0.475, z)
      scene.add(body)
      const bottle = new THREE.Mesh(new THREE.CylinderGeometry(0.14, 0.14, 0.42, 20), glass)
      bottle.position.set(x, 1.18, z)
      scene.add(bottle)
      const neck = new THREE.Mesh(new THREE.CylinderGeometry(0.06, 0.06, 0.06, 12), glass)
      neck.position.set(x, 0.98, z)
      scene.add(neck)
      const tap = new THREE.Mesh(new THREE.BoxGeometry(0.04, 0.04, 0.08), steel)
      tap.position.set(x, 0.72, z + 0.19)
      scene.add(tap)
    }

    /** A television on a stand, the screen toward +facing z. */
    const television = (x: number, z: number, facing: 1 | -1) => {
      const cab = models.instance('cabinetTelevision', { width: 1.5 })
      const realTv = photo('television', { width: 1.1 })
      if (cab && realTv) {
        const yaw = (facing === 1 ? 0 : Math.PI) + KENNEY_YAW
        place(cab, x, 0, z, yaw)
        place(realTv, x, cab.size.y, z, facing === 1 ? 0 : Math.PI)
        return
      }
      if (cab) {
        const yaw = (facing === 1 ? 0 : Math.PI) + KENNEY_YAW
        cab.group.position.set(x, 0, z)
        cab.group.rotation.y = yaw
        scene.add(cab.group)
        const tv = models.instance('televisionModern', { width: 1.25 })
        if (tv) {
          tv.group.position.set(x, cab.size.y, z)
          tv.group.rotation.y = yaw
          scene.add(tv.group)
        }
        return
      }
      const stand = shadowed(new THREE.Mesh(new THREE.BoxGeometry(1.4, 0.5, 0.4), equipment))
      stand.position.set(x, 0.25, z)
      scene.add(stand)
      const screen = new THREE.Mesh(new THREE.BoxGeometry(1.3, 0.75, 0.05), screenMaterial)
      screen.position.set(x, 0.92, z)
      scene.add(screen)
      // A price line on the screen, in the token that means "up".
      const points: THREE.Vector3[] = []
      for (let i = 0; i <= 12; i++) {
        points.push(new THREE.Vector3(x - 0.55 + i * 0.09, 0.75 + 0.18 * (0.5 + 0.5 * Math.sin(i * 1.3)) + i * 0.012, z + facing * 0.03))
      }
      const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(points), new THREE.LineBasicMaterial({ color: colors.primary }))
      scene.add(line)
    }

    /* ---- walls: flat, along the north and west edges ----
       Plain painted slabs with a skirting and a cornice, nothing on them
       yet: they are the surfaces the floor will decorate. Behind everything
       from where the camera looks, and casting no shadow so the west light
       still reaches the desks. */
    const wallPaint = new THREE.MeshPhysicalMaterial({ color: walnut, roughness: 0.6, clearcoat: 0.3, clearcoatRoughness: 0.4 })
    const wallJoint = new THREE.MeshBasicMaterial({ color: walnut.clone().lerp(colors.background, 0.45) })
    const skirtingMaterial = new THREE.MeshPhysicalMaterial({ color: colors.background.clone().lerp(colors.foreground, 0.1), roughness: 0.7 })
    const flatWall = (axis: 'x' | 'z', at: number, from: number, to: number) => {
      const H = 3.0
      const mid = (from + to) / 2
      const len = to - from
      const put = (geometry: THREE.BufferGeometry, material: THREE.Material, y: number) => {
        const m = new THREE.Mesh(geometry, material)
        m.receiveShadow = true
        if (axis === 'x') m.position.set(mid, y, at)
        else {
          m.position.set(at, y, mid)
          m.rotation.y = Math.PI / 2
        }
        scene.add(m)
      }
      put(new THREE.BoxGeometry(len, H, 0.22), wallPaint, H / 2)
      put(new THREE.BoxGeometry(len, 0.14, 0.26), skirtingMaterial, 0.07)
      put(new THREE.BoxGeometry(len, 0.1, 0.26), skirtingMaterial, H - 0.05)
      // Panel joints every 60 cm, so the wall reads as veneer panels.
      for (let p = from + 0.6; p < to; p += 0.6) {
        const j = new THREE.Mesh(new THREE.BoxGeometry(0.012, H - 0.24, 0.24), wallJoint)
        if (axis === 'x') j.position.set(p, H / 2, at)
        else {
          j.position.set(at, H / 2, p)
        }
        scene.add(j)
      }
    }
    flatWall('x', FLOOR_Z0 - 0.12, -FLOOR_W / 2 - 0.12, FLOOR_W / 2 + 0.12)
    flatWall('z', -FLOOR_W / 2 - 0.12, FLOOR_Z0 - 0.12, FLOOR_Z1 + 0.12)

    /* ---- linear lights: suspended LED bars in rows, as the ceiling would carry ---- */
    // Seen from above they are thin lines of light, nothing more: no housing
    // to cut across the view.
    const ledBar = new THREE.MeshBasicMaterial({ color: colors.foreground.clone().lerp(colors.caution, 0.2) })
    for (const rowZ of [FLOOR_Z0 + 2.4, AISLE_Z - 2.0, AISLE_Z + 2.2]) {
      for (let bx = -FLOOR_W / 2 + 2.2; bx < FLOOR_W / 2 - 1.5; bx += 3.6) {
        const bar = new THREE.Mesh(new THREE.BoxGeometry(2.4, 0.015, 0.04), ledBar)
        bar.position.set(bx, 3.05, rowZ)
        scene.add(bar)
        const soft = haloFor(colors.foreground.clone().lerp(colors.caution, 0.3), 0.7)
        soft.material.opacity = 0.12
        soft.position.set(bx, 3.0, rowZ)
      }
    }

    /* ---- clusters ---- */
    const clusters: Cluster[] = []
    for (const dept of departments) {
      const tone = toneColor(dept.tone)
      const rugMaterial = new THREE.MeshPhysicalMaterial({
        color: carpet.clone().lerp(tone, 0.04),
        roughness: 0.95,
        emissive: tone.clone(),
        emissiveIntensity: 0,
      })
      const mat = new THREE.Mesh(rug(dept.w, dept.d), rugMaterial)
      mat.position.set(dept.x, 0.006, dept.z)
      mat.receiveShadow = true
      scene.add(mat)
      // The department's colour, as a thin line along the aisle edge of its carpet.
      const accent = new THREE.Mesh(new THREE.BoxGeometry(dept.w - 0.5, 0.012, 0.06), new THREE.MeshBasicMaterial({ color: tone }))
      accent.position.set(dept.x, 0.014, dept.z + (dept.z < AISLE_Z ? 1 : -1) * (dept.d / 2 - 0.1))
      scene.add(accent)
      rugMeshes.push(mat)

      const facing: 1 | -1 = dept.z < AISLE_Z ? 1 : -1 // toward the aisle
      const centre = new THREE.Vector3(dept.x, 0, dept.z)
      const cluster: Cluster = {
        dept,
        centre,
        aisle: new THREE.Vector3(dept.x, 0, AISLE_Z),
        rugMaterial,
        rug: mat,
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
          const rail = new THREE.Mesh(new THREE.BoxGeometry(w + 0.02, 0.05, d + 0.03), frameMaterial)
          rail.position.set(px, GLASS_H + 0.025, pz)
          scene.add(rail)
          const sill = new THREE.Mesh(new THREE.BoxGeometry(w + 0.02, 0.06, d + 0.03), frameMaterial)
          sill.position.set(px, 0.03, pz)
          scene.add(sill)
          // Slim black mullions every 90 cm.
          const along = Math.max(w, d)
          for (let m = 0.9; m < along - 0.3; m += 0.9) {
            // A slim post: 3.5 cm along the panel, the panel's thickness across it.
            const mullion = new THREE.Mesh(new THREE.BoxGeometry(w > d ? 0.035 : w + 0.02, GLASS_H, w > d ? d + 0.02 : 0.035), frameMaterial)
            if (w > d) mullion.position.set(px - w / 2 + m, GLASS_H / 2, pz)
            else mullion.position.set(px, GLASS_H / 2, pz - d / 2 + m)
            scene.add(mullion)
          }
        }
        // A walnut panel to sill height on the back, glass above it: the
        // room reads as wood-and-glass without turning into a dark box.
        const woodWall = (px: number, pz: number, w: number, d: number) => {
          const h = 1.1
          const m = shadowed(new THREE.Mesh(new THREE.BoxGeometry(w, h, d), wallPaint))
          m.position.set(px, h / 2, pz)
          scene.add(m)
          const cap = new THREE.Mesh(new THREE.BoxGeometry(w + 0.02, 0.04, d + 0.04), frameMaterial)
          cap.position.set(px, h + 0.02, pz)
          scene.add(cap)
        }
        const w = x1 - x0
        const d = z1 - z0
        const doorZ = facing === 1 ? z1 : z0
        const backZ = facing === 1 ? z0 : z1
        // Glass all round; walnut to sill height along the back.
        panel((x0 + x1) / 2, backZ, w, 0.04)
        woodWall((x0 + x1) / 2, backZ + facing * 0.09, w - 0.1, 0.12)
        panel(x0, (z0 + z1) / 2, 0.04, d)
        panel(x1, (z0 + z1) / 2, 0.04, d)
        const doorW = 1.0
        const side = (w - doorW) / 2
        panel(x0 + side / 2, doorZ, side, 0.04)
        panel(x1 - side / 2, doorZ, side, 0.04)
        for (const [px, pz] of [[x0, z0], [x1, z0], [x0, z1], [x1, z1]] as [number, number][]) {
          const postMesh = new THREE.Mesh(new THREE.BoxGeometry(0.06, GLASS_H + 0.05, 0.06), frameMaterial)
          postMesh.position.set(px, (GLASS_H + 0.05) / 2, pz)
          scene.add(postMesh)
        }
        palm(x0 + 0.5, backZ + facing * 0.5, 0.85)
        plant(x1 + 0.4, doorZ - facing * 0.15, 0.9)
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
        // A guard on the front of the row, where the cabinets face: rope
        // stanchions along the edge, and the guard walking a beat between
        // the ends of the row behind the rope.
        const frontZ = dept.z - facing * (dept.d / 2 + 0.05)
        const postTops: THREE.Vector3[] = []
        for (const dx of [-1.1, 1.1]) {
          const base = new THREE.Mesh(new THREE.CylinderGeometry(0.14, 0.14, 0.03, 20), frameMaterial)
          base.position.set(dept.x + dx, 0.015, frontZ)
          scene.add(base)
          const post = shadowed(new THREE.Mesh(new THREE.CylinderGeometry(0.025, 0.025, 0.95, 12), frameMaterial))
          post.position.set(dept.x + dx, 0.5, frontZ)
          scene.add(post)
          const knob = new THREE.Mesh(new THREE.SphereGeometry(0.045, 12, 12), frameMaterial)
          knob.position.set(dept.x + dx, 0.99, frontZ)
          scene.add(knob)
          postTops.push(new THREE.Vector3(dept.x + dx, 0.93, frontZ))
        }
        const sag = postTops[0].clone().lerp(postTops[1], 0.5)
        sag.y -= 0.18
        const rope = shadowed(new THREE.Mesh(new THREE.TubeGeometry(new THREE.QuadraticBezierCurve3(postTops[0], sag, postTops[1]), 16, 0.018, 8, false), new THREE.MeshPhysicalMaterial({ color: colors.caution.clone().lerp(colors.background, 0.3), roughness: 0.8 })))
        scene.add(rope)
        const beatZ = frontZ + facing * 0.45
        const guard = person(dept.x - 1.0, beatZ, facing === 1 ? Math.PI : 0, new THREE.MeshPhysicalMaterial({ color: steelBlack, roughness: 0.7 }), { kind: 'standing' })
        const post = agents.find((a) => a.group === guard)
        if (post) {
          post.restless = 1
          post.dwell = rand(2, 5)
          post.beat = new THREE.Vector3(dept.x + 1.0, 0, beatZ)
        }
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
      } else if (dept.furniture === 'showcase') {
        // The strip along the long wall: the show car in the middle against
        // the wall, the front desk to the west of it, a sofa group either
        // side, coffee and the screen to the east, plants at the ends.
        const cx = dept.x
        const cz = dept.z - 0.75
        // A single-seater is three body-lengths long next to these people.
        const CAR_LENGTH = 4.0
        // The car, centre of the long wall, on its plinth under its own light.
        const plinth = shadowed(new THREE.Mesh(new THREE.CylinderGeometry(2.0, 2.05, 0.12, 64), frameMaterial))
        plinth.position.set(cx, 0.06, cz)
        scene.add(plinth)
        const plinthTop = new THREE.Mesh(new THREE.CylinderGeometry(1.95, 1.95, 0.02, 64), equipment)
        plinthTop.position.set(cx, 0.13, cz)
        scene.add(plinthTop)
        // The car stands in a holder that turns; the primitive car fills it
        // now and the model replaces it when it arrives.
        const holder = new THREE.Group()
        holder.position.set(cx, 0.14, cz)
        holder.rotation.y = 0.5
        scene.add(holder)
        const standIn = raceCar()
        standIn.scale.setScalar(CAR_LENGTH / 2.7)
        holder.add(standIn)
        showCar = holder
        if (carModel) {
          const loader = new GLTFLoader()
          loader.setMeshoptDecoder(MeshoptDecoder)
          loader.load(
            carModel,
            (gltf) => {
              if (disposed) return
              const model = gltf.scene
              model.traverse((o) => {
                if (o instanceof THREE.Mesh) {
                  o.castShadow = true
                  o.receiveShadow = true
                }
              })
              // Fit the longest side to the plinth and stand it on the top.
              const box = new THREE.Box3().setFromObject(model)
              const size = box.getSize(new THREE.Vector3())
              const longest = Math.max(size.x, size.z) || 1
              model.scale.setScalar(CAR_LENGTH / longest)
              box.setFromObject(model)
              const centre = box.getCenter(new THREE.Vector3())
              model.position.set(-centre.x, -box.min.y, -centre.z)
              // The model's nose points along −z after Sketchfab's export;
              // turn it to run along the plinth like the stand-in did.
              if (size.z > size.x) model.rotation.y = Math.PI / 2
              holder.remove(standIn)
              disposeObject(standIn)
              holder.add(model)
            },
            undefined,
            () => {
              /* the stand-in stays; a missing model is not an error worth a console line */
            },
          )
        }
        const carLight = new THREE.SpotLight(colors.foreground, 6, 9, 0.6, 0.7, 1.2)
        carLight.position.set(cx, 4.6, cz)
        carLight.target = holder
        carLight.castShadow = true
        carLight.shadow.mapSize.set(1024, 1024)
        scene.add(carLight)
        const carHalo = haloFor(colors.caution, 2.4)
        carHalo.material.opacity = 0.18
        carHalo.position.set(cx, 0.5, cz)
        // The front desk, west of the car, facing the floor.
        const counter = shadowed(new THREE.Mesh(new THREE.BoxGeometry(2.2, 1.0, 0.55), deskTop))
        counter.position.set(dept.x - dept.w / 2 + 1.7, 0.5, dept.z + 1.9)
        scene.add(counter)
        const counterTop = new THREE.Mesh(new THREE.BoxGeometry(2.3, 0.04, 0.65), frameMaterial)
        counterTop.position.set(counter.position.x, 1.02, counter.position.z)
        scene.add(counterTop)
        const counterScreen = new THREE.Mesh(new THREE.BoxGeometry(0.34, 0.22, 0.02), screenMaterial)
        counterScreen.position.set(counter.position.x + 0.5, 1.16, counter.position.z - 0.1)
        scene.add(counterScreen)
        person(counter.position.x, counter.position.z - 0.75, 0, new THREE.MeshPhysicalMaterial({ color: colors.foreground.clone().lerp(colors.muted, 0.2), roughness: 0.6 }), { kind: 'standing' })
        const rack = models.instance('coatRackStanding', { height: 1.7 })
        if (rack) {
          rack.group.position.set(dept.x - dept.w / 2 + 0.5, 0, dept.z - 1.9)
          scene.add(rack.group)
        }
        // Sofa groups either side of the car, facing each other across a table.
        const group = (gx: number) => {
          if (real.has(PHOTOREAL.loungeSofa.name)) {
            for (const f of [1, -1] as const) place(photo('loungeSofa', { width: 2.0 }), gx, 0, dept.z - f * 1.15, f === 1 ? 0 : Math.PI)
            place(photo('tableCoffee', { width: 1.0 }), gx, 0, dept.z)
            return
          }
          for (const f of [1, -1] as const) {
            const inst = models.instance('loungeSofa', { width: 1.9 })
            if (!inst) break
            inst.group.position.set(gx, 0, dept.z - f * 1.15)
            inst.group.rotation.y = (f === 1 ? 0 : Math.PI) + KENNEY_YAW
            scene.add(inst.group)
          }
          const table = models.instance('tableCoffee', { width: 1.0 })
          if (table) {
            table.group.position.set(gx, 0, dept.z)
            scene.add(table.group)
          }
        }
        group(dept.x - 4.6)
        group(dept.x + 4.6)
        person(dept.x + 4.6 - 0.45, dept.z - 1.15, 0, new THREE.MeshPhysicalMaterial({ color: colors.muted.clone().lerp(colors.foreground, 0.1), roughness: 0.6 }), { kind: 'seated', seat: 0.42, armsOnDesk: false })
        person(dept.x - 2.3, dept.z + 0.9, Math.PI / 2 + 0.4, new THREE.MeshPhysicalMaterial({ color: colors.mint.clone().lerp(colors.background, 0.35), roughness: 0.6 }), { kind: 'standing' })
        // Coffee and the screen at the east end.
        if (!place(photo('coffeeCart', { height: 1.1 }), dept.x + dept.w / 2 - 1.6, 0, dept.z - 1.8, Math.PI)) {
          coffeeStation(dept.x + dept.w / 2 - 1.6, dept.z - 1.8, 1)
        }
        waterCooler(dept.x + dept.w / 2 - 0.5, dept.z - 1.8)
        television(dept.x + dept.w / 2 - 1.6, dept.z + 1.9, 1)
        floorLamp(dept.x - dept.w / 2 + 0.5, dept.z + 2.2)
        floorLamp(dept.x + 2.6, dept.z + 2.2)
        palm(dept.x - dept.w / 2 + 0.6, dept.z - dept.d / 2 + 0.6, 1.0)
        palm(dept.x + dept.w / 2 - 0.6, dept.z - dept.d / 2 + 0.6, 1.0)
        plant(cx - 2.8, cz + 0.9, 1.05)
        plant(cx + 2.8, cz + 0.9, 1.05)
      } else if (dept.furniture === 'meeting') {
        // A long table, three chairs a side, a screen on the back glass.
        const tableW = dept.w - 1.2
        if (!place(photo('table', { width: tableW }), dept.x, 0, dept.z)) {
          const table = shadowed(new THREE.Mesh(new THREE.BoxGeometry(tableW, 0.05, 0.95), deskTop))
          table.position.set(dept.x, 0.72, dept.z)
          scene.add(table)
          for (const sx of [-1, 1]) {
            const legSlab = new THREE.Mesh(new THREE.BoxGeometry(0.06, 0.7, 0.8), deskLeg)
            legSlab.position.set(dept.x + (sx * (tableW - 0.4)) / 2, 0.35, dept.z)
            scene.add(legSlab)
          }
        }
        for (let i = 0; i < 3; i++) {
          const cx = dept.x - (tableW - 0.9) / 2 + (i * (tableW - 0.9)) / 2
          chair(cx, dept.z - 0.85, 1)
          chair(cx, dept.z + 0.85, -1)
        }
        const screen = new THREE.Mesh(new THREE.BoxGeometry(1.2, 0.68, 0.04), screenMaterial)
        screen.position.set(dept.x, 1.1, dept.z - facing * (dept.d / 2 + 0.16))
        scene.add(screen)
        const credenza = shadowed(new THREE.Mesh(new THREE.BoxGeometry(1.4, 0.55, 0.4), equipment))
        credenza.position.set(dept.x - dept.w / 2 + 0.9, 0.275, dept.z - facing * (dept.d / 2 - 0.25))
        scene.add(credenza)
        const jug = new THREE.Mesh(new THREE.CylinderGeometry(0.07, 0.06, 0.22, 16), glass)
        jug.position.set(dept.x + 0.3, 0.86, dept.z)
        scene.add(jug)
        const laptop = models.instance('laptop', { width: 0.36 })
        if (laptop) {
          laptop.group.position.set(dept.x - 0.5, 0.745, dept.z + 0.1)
          laptop.group.rotation.y = Math.PI + KENNEY_YAW
          scene.add(laptop.group)
        }
        for (const dx of [-0.6, 0.6]) {
          const pad = new THREE.Mesh(new THREE.BoxGeometry(0.22, 0.01, 0.3), paper)
          pad.position.set(dept.x + dx, 0.75, dept.z - 0.2)
          scene.add(pad)
        }
        whiteboard(dept.x + dept.w / 2 - 0.35, dept.z, 1)
        plant(dept.x - dept.w / 2 + 0.4, dept.z + facing * (dept.d / 2 - 0.4), 0.9)
      } else if (dept.furniture === 'shelves' && real.has(PHOTOREAL.shelves.name)) {
        // Two rows of units. A rug deeper than it is wide (the corner) runs
        // them along z with their fronts toward the floor; otherwise along x.
        const alongZ = dept.d > dept.w
        const run = (alongZ ? dept.d : dept.w) - 0.3
        const n = Math.max(1, Math.floor(run / 1.45))
        for (let i = 0; i < 2; i++) {
          for (let k = 0; k < n; k++) {
            const along = -((n - 1) * 1.45) / 2 + k * 1.45
            if (alongZ) {
              place(photo('shelves', { width: 1.4 }), dept.x - 0.65 + i * 1.3, 0, dept.z + along, -Math.PI / 2)
            } else {
              place(photo('shelves', { width: 1.4 }), dept.x + along, 0, dept.z - facing * (0.75 - i * 1.3), facing === 1 ? 0 : Math.PI)
            }
          }
        }
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

      // What each department keeps beside its desks.
      const back = dept.z - facing * (dept.d / 2 - 0.35)
      switch (dept.id) {
        case 'arbiter': {
          bookcase(dept.x, back, dept.w - 0.9, facing)
          floorLamp(dept.x + dept.w / 2 - 0.4, dept.z + facing * (dept.d / 2 - 0.4))
          // Two guest chairs across the desk, and a clock.
          for (const dx of [-0.5, 0.5]) {
            if (!place(photo('armChair', { height: 0.85 }), dept.x + dx, 0, dept.z + facing * 0.9, facing === 1 ? Math.PI : 0)) {
              chair(dept.x + dx, dept.z + facing * 0.85, (-facing as 1 | -1))
            }
          }
          place(photo('clock', { height: 0.3 }), dept.x + dept.w / 2 - 0.5, 1.15, back, facing === 1 ? 0 : Math.PI)
          break
        }
        case 'advisory': {
          whiteboard(dept.x - dept.w / 2 + 0.9, back, facing)
          cabinet(dept.x + dept.w / 2 - 0.4, back, facing)
          break
        }
        case 'archive': {
          // A reading table with a lamp, at the aisle side.
          const table = shadowed(new THREE.Mesh(new THREE.BoxGeometry(1.2, 0.05, 0.6), deskTop))
          table.position.set(dept.x - dept.w / 2 + 1.0, 0.72, dept.z + facing * (dept.d / 2 - 0.6))
          scene.add(table)
          for (const dx of [-0.5, 0.5]) {
            const leg = new THREE.Mesh(new THREE.BoxGeometry(0.05, 0.7, 0.5), deskLeg)
            leg.position.set(table.position.x + dx, 0.35, table.position.z)
            scene.add(leg)
          }
          deskLamp(table.position.x + 0.4, 0.74, table.position.z - facing * 0.15)
          chair(table.position.x, table.position.z + facing * 0.6, (-facing as 1 | -1))
          break
        }
        case 'data': {
          // Air handler and an extinguisher on the aisle side, behind the
          // cabinets, clear of the guard's beat along the front.
          const rear = dept.z + facing * (dept.d / 2 - 0.35)
          const ac = shadowed(new THREE.Mesh(new THREE.BoxGeometry(0.7, 1.9, 0.5), paper))
          ac.position.set(dept.x + dept.w / 2 - 0.5, 0.95, rear)
          scene.add(ac)
          for (let i = 0; i < 6; i++) {
            const slat = new THREE.Mesh(new THREE.BoxGeometry(0.5, 0.015, 0.02), frameMaterial)
            slat.position.set(ac.position.x, 1.35 + i * 0.07, rear - facing * 0.26)
            scene.add(slat)
          }
          const ext = new THREE.Mesh(new THREE.CylinderGeometry(0.07, 0.07, 0.4, 12), new THREE.MeshPhysicalMaterial({ color: colors.caution, roughness: 0.4 }))
          ext.position.set(dept.x - dept.w / 2 + 0.3, 0.2, rear)
          scene.add(ext)
          break
        }
        case 'lab': {
          whiteboard(dept.x + dept.w / 2 - 0.7, back, facing)
          break
        }
        case 'engine': {
          // Two cabinets flank the core, a fan on each, cables to the tray.
          for (const sx of [-1, 1]) {
            const cab = shadowed(new THREE.Mesh(new THREE.BoxGeometry(0.5, 1.5, 0.6), equipment))
            cab.position.set(dept.x + sx * 1.35, 0.75, back + facing * 0.1)
            scene.add(cab)
            const fan = new THREE.Mesh(new THREE.CylinderGeometry(0.17, 0.17, 0.03, 24), steel)
            fan.position.set(cab.position.x, 1.15, cab.position.z + facing * 0.31)
            fan.rotation.x = Math.PI / 2
            scene.add(fan)
            fans.push(fan)
            for (let k = 0; k < 4; k++) {
              const on = k === 3 ? colors.caution.clone() : colors.primary.clone()
              const material = new THREE.MeshBasicMaterial({ color: colors.background })
              const dot = new THREE.Mesh(ledGeometry, material)
              dot.position.set(cab.position.x - 0.12 + k * 0.08, 0.7, cab.position.z + facing * 0.305)
              scene.add(dot)
              const halo = haloFor(on, 0.1)
              halo.position.copy(dot.position).add(new THREE.Vector3(0, 0, facing * 0.03))
              const lit = Math.random() < 0.5
              material.color.copy(lit ? on : colors.background.clone().lerp(on, 0.1))
              halo.material.opacity = lit ? 0.7 : 0
              leds.push({ material, halo, on, off: colors.background.clone().lerp(on, 0.1), rate: k === 3 ? 0.5 : 3 + Math.random() * 6, lit })
            }
          }
          const coreGlow = haloFor(colors.primary, 1.4)
          coreGlow.material.opacity = 0.16
          coreGlow.position.set(dept.x, 0.6, dept.z + facing * 0.3)
          break
        }
        case 'data-integrity':
        case 'adversary':
        case 'risk': {
          deskLamp(dept.x + 0.42, 0.74, dept.z + facing * 0.05)
          // Beside the desk, not behind the chair: the camera looks from the south.
          cabinet(dept.x - 0.62, dept.z + facing * 0.45, facing)
          break
        }
        default:
          break
      }
      clusters.push(cluster)
    }

    // Plants along the edge, where an open plan keeps them.
    palm(FLOOR_W / 2 - 0.8, FLOOR_Z1 - 0.8, 1.0)
    palm(-FLOOR_W / 2 + 0.8, FLOOR_Z1 - 0.8, 0.95)
    plant(FLOOR_W / 2 - 0.7, AISLE_Z + 0.2, 1.0)

    /* ---- where people go, and how they get there ---- */
    const WALKWAY_Z = 0.05
    const lanes = { aisle: AISLE_Z, walkway: WALKWAY_Z }
    const laneOf = (z: number) => (z < (WALKWAY_Z + AISLE_Z) / 2 ? lanes.walkway : lanes.aisle)
    // The gaps between north-wing clusters, where one lane meets the other.
    const northWing = clusters.filter((c) => Math.abs(c.dept.z - 2.3) < 0.5).sort((a, b) => a.dept.x - b.dept.x)
    const connectors: number[] = [-FLOOR_W / 2 + 0.7]
    for (let i = 0; i + 1 < northWing.length; i++) connectors.push((northWing[i].dept.x + northWing[i].dept.w / 2 + northWing[i + 1].dept.x - northWing[i + 1].dept.w / 2) / 2)
    connectors.push(FLOOR_W / 2 - 0.7)
    const routeTo = (from: THREE.Vector3, to: THREE.Vector3): THREE.Vector3[] => {
      const a = laneOf(from.z)
      const b = laneOf(to.z)
      const path = [new THREE.Vector3(from.x, 0, a)]
      if (a !== b) {
        const mid = (from.x + to.x) / 2
        const cx = connectors.reduce((best, c) => (Math.abs(c - mid) < Math.abs(best - mid) ? c : best), connectors[0])
        path.push(new THREE.Vector3(cx, 0, a), new THREE.Vector3(cx, 0, b))
      }
      path.push(new THREE.Vector3(to.x, 0, b), to.clone().setY(0))
      return path
    }
    // Somewhere to go: the front of each cluster, the coffee, the water, the car.
    const pois: { at: THREE.Vector3; yaw: number }[] = []
    for (const c of clusters) {
      if (c.dept.furniture === 'showcase' || c.dept.enclosed) continue
      const f = c.dept.z < AISLE_Z ? 1 : -1
      pois.push({ at: new THREE.Vector3(c.dept.x + rand(-0.6, 0.6), 0, c.dept.z + f * (c.dept.d / 2 + 0.45)), yaw: f === 1 ? Math.PI : 0 })
    }
    const showcase = clusters.find((c) => c.dept.furniture === 'showcase')
    if (showcase) {
      const d = showcase.dept
      pois.push({ at: new THREE.Vector3(d.x + d.w / 2 - 1.6, 0, d.z - 0.7), yaw: Math.PI })
      pois.push({ at: new THREE.Vector3(d.x + d.w / 2 - 0.5, 0, d.z - 0.6), yaw: Math.PI })
      pois.push({ at: new THREE.Vector3(d.x - 2.9, 0, d.z - 0.4), yaw: Math.PI / 2 })
      pois.push({ at: new THREE.Vector3(d.x + 2.9, 0, d.z - 0.4), yaw: -Math.PI / 2 })
      pois.push({ at: new THREE.Vector3(d.x - d.w / 2 + 1.7, 0, d.z + 2.9), yaw: Math.PI })
    }
    const WALK_SPEED = 1.25
    const fadeTo = (agent: Agent, next: 'sit' | 'idle' | 'walk') => {
      if (agent.current === next) return
      const from = agent.actions[agent.current]
      const to = agent.actions[next]
      if (!to) return
      to.enabled = true
      to.reset()
      to.setEffectiveWeight(1)
      to.play()
      if (from) from.crossFadeTo(to, 0.3, false)
      agent.current = next
    }
    const startTrip = (agent: Agent, goal: 'home' | 'away') => {
      const from = agent.group.position.clone()
      const to = goal === 'home' ? new THREE.Vector3(agent.home.x, 0, agent.home.z) : agent.beat ? agent.beat : pois[Math.floor(Math.random() * pois.length)].at
      // A beat is a straight line; everyone else takes the lanes.
      agent.path = agent.beat ? [to.clone().setY(0)] : routeTo(from, to)
      agent.leg = 0
      agent.goal = goal
      agent.state = 'walking'
      agent.group.position.y = 0
      fadeTo(agent, 'walk')
    }
    const updateAgents = (dt: number) => {
      for (const agent of agents) {
        if (agent.state !== 'walking') {
          agent.dwell -= dt
          if (agent.dwell > 0) continue
          if (agent.state === 'home') {
            if (Math.random() < agent.restless && pois.length > 0) startTrip(agent, 'away')
            else agent.dwell = agent.home.seated ? rand(40, 120) : rand(8, 30)
          } else startTrip(agent, 'home')
          continue
        }
        const target = agent.path[agent.leg]
        if (!target) {
          agent.state = agent.goal
          if (agent.goal === 'home') {
            agent.group.position.set(agent.home.x, agent.home.y, agent.home.z)
            agent.group.rotation.y = agent.home.yaw
            fadeTo(agent, agent.home.seated ? 'sit' : 'idle')
            agent.dwell = agent.beat ? rand(2, 5) : agent.home.seated ? rand(60, 180) : rand(10, 40)
          } else {
            fadeTo(agent, 'idle')
            agent.dwell = agent.home.seated ? rand(3, 8) : rand(6, 18)
            if (agent.beat) {
              // At the end of the beat: face out, a short stand, then back.
              agent.group.rotation.y = agent.home.yaw
              agent.dwell = rand(2, 5)
            }
          }
          continue
        }
        const pos = agent.group.position
        const dx = target.x - pos.x
        const dz = target.z - pos.z
        const dist = Math.hypot(dx, dz)
        const step = WALK_SPEED * dt
        if (dist <= step) {
          pos.set(target.x, 0, target.z)
          agent.leg += 1
          continue
        }
        pos.x += (dx / dist) * step
        pos.z += (dz / dist) * step
        // Face the way of travel; +z is the character's front.
        const want = Math.atan2(dx, dz)
        let delta = want - agent.group.rotation.y
        delta = Math.atan2(Math.sin(delta), Math.cos(delta))
        agent.group.rotation.y += delta * Math.min(1, dt * 10)
      }
    }

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

    /* ---- the view: click a department to fly to it, double-click to come back ---- */
    const home = { position: camera.position.clone(), target: controls.target.clone() }
    let goal: { position: THREE.Vector3; target: THREE.Vector3 } | null = null
    let focused = false
    let hovered: Cluster | null = null
    const raycaster = new THREE.Raycaster()
    const pointer = new THREE.Vector2()
    const pressed = { x: 0, y: 0 }

    const pick = (event: PointerEvent): Cluster | null => {
      const rect = renderer.domElement.getBoundingClientRect()
      pointer.set(((event.clientX - rect.left) / rect.width) * 2 - 1, -((event.clientY - rect.top) / rect.height) * 2 + 1)
      raycaster.setFromCamera(pointer, camera)
      const hit = raycaster.intersectObjects(rugMeshes, false)[0]
      return hit ? (clusters.find((c) => c.rug === hit.object) ?? null) : null
    }
    const flyTo = (c: Cluster) => {
      // Close enough to read the desks, from the same side the floor is seen.
      const span = Math.max(c.dept.w, c.dept.d)
      const distance = span * 1.35 + 4.5
      const from = new THREE.Vector3(0.5, 0.72, 0.85).normalize()
      const target = c.centre.clone().setY(0.6)
      goal = { target, position: target.clone().add(from.multiplyScalar(distance)) }
      controls.autoRotate = false
      setZoomed(true)
    }
    const resetView = () => {
      goal = { position: home.position.clone(), target: home.target.clone() }
      controls.autoRotate = !reduceMotion
      setZoomed(false)
    }
    resetRef.current = resetView

    const onWheel = (event: WheelEvent) => {
      controls.enableZoom = focused || event.ctrlKey || event.metaKey
      if (controls.enableZoom) {
        goal = null
        controls.autoRotate = false
        setZoomed(true)
      }
    }
    const onPointerDown = (event: PointerEvent) => {
      focused = true
      pressed.x = event.clientX
      pressed.y = event.clientY
    }
    let pickTimer = 0
    const onPointerUp = (event: PointerEvent) => {
      // A drag orbits; a click (no travel) picks — after a beat, so the two
      // clicks of a double-click do not fly somewhere before the reset.
      if (Math.hypot(event.clientX - pressed.x, event.clientY - pressed.y) > 5) return
      const c = pick(event)
      window.clearTimeout(pickTimer)
      if (c) pickTimer = window.setTimeout(() => flyTo(c), 260)
    }
    const onPointerMove = (event: PointerEvent) => {
      hovered = pick(event)
      host.style.cursor = hovered ? 'pointer' : 'grab'
    }
    const onPointerLeave = () => {
      focused = false
      hovered = null
      controls.enableZoom = false
    }
    const onDoubleClick = () => {
      window.clearTimeout(pickTimer)
      resetView()
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') resetView()
    }
    host.addEventListener('wheel', onWheel, { capture: true, passive: true })
    host.addEventListener('pointerdown', onPointerDown)
    host.addEventListener('pointerup', onPointerUp)
    host.addEventListener('pointermove', onPointerMove)
    host.addEventListener('pointerleave', onPointerLeave)
    host.addEventListener('dblclick', onDoubleClick)
    host.addEventListener('keydown', onKey)
    host.tabIndex = 0
    host.style.cursor = 'grab'
    host.style.outline = 'none'

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
        for (const fan of fans) fan.rotation.y += dt * 9
        updateAgents(dt)
        for (const m of mixers) m.update(dt)
        // People: a look around now and then, and the typists' hands moving.
        for (const p of people) {
          p.head.rotation.y = Math.sin(now * 0.32 + p.phase) * 0.22
          p.head.rotation.x = Math.sin(now * 0.21 + p.phase * 2) * 0.05
          if (p.typing) {
            p.hands[0].position.y = 0.77 + Math.max(0, Math.sin(now * 9 + p.phase)) * 0.012
            p.hands[1].position.y = 0.77 + Math.max(0, Math.sin(now * 9 + p.phase + 1.9)) * 0.012
          }
        }

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

      if (goal) {
        // Exponential ease toward the goal, frame-rate independent.
        const k = 1 - Math.pow(0.002, dt)
        camera.position.lerp(goal.position, k)
        controls.target.lerp(goal.target, k)
        if (camera.position.distanceTo(goal.position) < 0.03 && controls.target.distanceTo(goal.target) < 0.03) goal = null
      }
      if (hovered) hovered.glow = Math.max(hovered.glow, 0.3)

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
        projected.y = c.dept.enclosed ? GLASS_H + 0.5 : c.dept.furniture === 'racks' ? 2.55 : c.dept.furniture === 'showcase' ? 2.4 : 1.75
        projected.project(camera)
        c.label.style.opacity = projected.z > 1 ? '0' : '1'
        c.label.style.left = `${((projected.x + 1) / 2) * width}px`
        c.label.style.top = `${((1 - projected.y) / 2) * height}px`
      }
      frame = requestAnimationFrame(render)
    }
    frame = requestAnimationFrame(render)

    teardown = () => {
      disposed = true
      window.clearTimeout(pickTimer)
      host.removeEventListener('wheel', onWheel, { capture: true })
      host.removeEventListener('pointerdown', onPointerDown)
      host.removeEventListener('pointerup', onPointerUp)
      host.removeEventListener('pointermove', onPointerMove)
      host.removeEventListener('pointerleave', onPointerLeave)
      host.removeEventListener('dblclick', onDoubleClick)
      host.removeEventListener('keydown', onKey)
      host.style.cursor = ''
      cancelAnimationFrame(frame)
      for (const t of timers) window.clearTimeout(t)
      observer.disconnect()
      controls.dispose()
      scene.environment?.dispose()
      disposeObject(scene)
      ringGeometry.dispose()
      ledGeometry.dispose()
      glow.dispose()
      for (const c of clusters) c.label.remove()
      for (const tag of rackTags) tag.label.remove()
    }
    } // build

    return () => {
      cancelled = true
      teardown()
      renderer.dispose()
      renderer.domElement.remove()
    }
  }, [departments, animate, carModel, modelsBase, photorealBase])

  return (
    <div
      className={className}
      role="img"
      aria-label="The company as an open-plan floor: desks in clusters on either side of one aisle, and a single glass office in the corner for the arbiter. Data racks, the strategy desk, the engine bay and the three veto desks sit south of the aisle; the advisory desks and the archive shelves north of it. A lit file walks the aisle from the archive through the lab and the engine, past each veto desk — any of which can stamp it amber and send it back — into the glass office, and back to the archive."
    >
      <div ref={container} className="absolute inset-0 [&>canvas]:block [&>canvas]:h-full [&>canvas]:w-full" />
      <div ref={labels} className="pointer-events-none absolute inset-0" aria-hidden />
      {zoomed && (
        <button
          type="button"
          onClick={() => resetRef.current()}
          className="bg-background/70 border-border hover:bg-accent/60 absolute top-3 right-4 rounded-full border px-2.5 py-1 text-[11px] backdrop-blur transition-colors"
          title="Back to the whole floor (double-click or Esc)"
        >
          ↩ whole floor
        </button>
      )}
    </div>
  )
}
