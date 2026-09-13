/**
 * The company as a floor plan.
 *
 * One floor, glass-walled rooms, a corridor down the middle. Each department
 * is a room; each agent is a figure at a desk in one. The thing that moves is
 * a *hypothesis*: a lit document that leaves the archive, is written up in
 * the strategy lab, run in the engine room, and then walks the veto wing —
 * three rooms in a row, any one of which can stamp it and send it back to
 * the archive as closed. What passes all three reaches the arbiter's corner
 * office, and from there goes to the archive as a record. Advisory desks
 * send their notes to the arbiter's office while a file is on the desk.
 *
 * The rooms are the research loop's steps (`.claude/skills/research/SKILL.md`)
 * and the figures are the agents (`.claude/agents/`); nothing here is
 * decoration for its own sake. A department with no agent — data, engine,
 * archive — is drawn with its equipment instead of a chair.
 *
 * Plain three.js in a ref; every resource disposed once. Labels are DOM,
 * projected. Colours are the CSS tokens, read at mount; the only colours made
 * in code are those tokens lightened or darkened.
 */

import { useEffect, useRef } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js'

export type Wing = 'north' | 'south'
export type Tone = 'arbiter' | 'veto' | 'advisory' | 'ops'

export interface Occupant {
  id: string
  title: string
  model: string
}

export interface Department {
  id: string
  /** Room name on the door. */
  title: string
  /** One line: what this room does. */
  line: string
  tone: Tone
  wing: Wing
  /** Relative width; rooms in a wing share the floor by these. */
  width: number
  occupants: Occupant[]
  /** Equipment drawn when nobody sits here. */
  furniture?: 'racks' | 'engine' | 'shelves'
}

interface Props {
  departments: Department[]
  className?: string
}

/* ---- geometry of the floor ---- */
const FLOOR_W = 17
const WING_DEPTH = 3.4
const CORRIDOR = 1.4
const GAP = 0.32
const WALL_H = 1.15
const WALL_T = 0.05

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

interface Room {
  dept: Department
  /** Floor rectangle, x/z extents. */
  x0: number
  x1: number
  z0: number
  z1: number
  centre: THREE.Vector3
  /** Where the corridor meets the door. */
  door: THREE.Vector3
  tile: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshPhysicalMaterial>
  tileColor: THREE.Color
  label: HTMLDivElement
  figures: Figure[]
  /** Room glow, 0..1, decays each frame. */
  glow: number
  glowColor: THREE.Color
}

interface Figure {
  occupant: Occupant
  group: THREE.Group
  material: THREE.MeshPhysicalMaterial
  position: THREE.Vector3
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
  to: Room
  color: THREE.Color
}

/** One stop on the hypothesis's route. */
interface Stop {
  room: Room
  /** Seconds the file rests on the desk. */
  dwell: number
  /** Chance this room sends it back. Only the veto wing has one. */
  vetoChance: number
}

/** Lay the rooms of a wing out left to right by relative width. */
function layout(departments: Department[]): Omit<Room, 'tile' | 'tileColor' | 'label' | 'figures' | 'glow' | 'glowColor'>[] {
  const out: Omit<Room, 'tile' | 'tileColor' | 'label' | 'figures' | 'glow' | 'glowColor'>[] = []
  for (const wing of ['north', 'south'] as Wing[]) {
    const rooms = departments.filter((d) => d.wing === wing)
    const total = rooms.reduce((s, d) => s + d.width, 0)
    const usable = FLOOR_W - GAP * (rooms.length + 1)
    let x = -FLOOR_W / 2 + GAP
    const z0 = wing === 'north' ? -CORRIDOR / 2 - WING_DEPTH : CORRIDOR / 2
    const z1 = z0 + WING_DEPTH
    for (const dept of rooms) {
      const w = (dept.width / total) * usable
      const x0 = x
      const x1 = x + w
      const cx = (x0 + x1) / 2
      const cz = (z0 + z1) / 2
      out.push({
        dept,
        x0,
        x1,
        z0,
        z1,
        centre: new THREE.Vector3(cx, 0, cz),
        door: new THREE.Vector3(cx, 0, wing === 'north' ? -CORRIDOR / 2 + 0.2 : CORRIDOR / 2 - 0.2),
      })
      x = x1 + GAP
    }
  }
  return out
}

/** A person: a capsule and a head, sat at a desk, facing the corridor. */
function buildFigure(material: THREE.Material): THREE.Group {
  const group = new THREE.Group()
  const body = new THREE.Mesh(new THREE.CapsuleGeometry(0.16, 0.34, 6, 18), material)
  body.position.y = 0.42
  body.castShadow = true
  const head = new THREE.Mesh(new THREE.SphereGeometry(0.13, 20, 20), material)
  head.position.y = 0.84
  head.castShadow = true
  group.add(body, head)
  return group
}

export function OfficeFloor({ departments, className }: Props) {
  const container = useRef<HTMLDivElement>(null)
  const labels = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const host = container.current
    const labelHost = labels.current
    if (!host || !labelHost) return

    const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches

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
      tone === 'arbiter' ? colors.primary : tone === 'veto' ? colors.caution : tone === 'ops' ? colors.mint : colors.foreground.clone().lerp(colors.muted, 0.35)

    /* ---- renderer ---- */
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, powerPreference: 'low-power' })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    renderer.setClearColor(0x000000, 0)
    renderer.outputColorSpace = THREE.SRGBColorSpace
    renderer.toneMapping = THREE.ACESFilmicToneMapping
    renderer.toneMappingExposure = 1.05
    renderer.shadowMap.enabled = true
    renderer.shadowMap.type = THREE.PCFSoftShadowMap
    host.appendChild(renderer.domElement)

    const scene = new THREE.Scene()
    scene.fog = new THREE.Fog(colors.card, 24, 44)

    const pmrem = new THREE.PMREMGenerator(renderer)
    scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture
    scene.environmentIntensity = 0.5
    pmrem.dispose()

    const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 120)
    camera.position.set(9, 13, 17)

    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableZoom = false
    controls.enablePan = false
    controls.enableDamping = true
    controls.dampingFactor = 0.06
    controls.autoRotate = !reduceMotion
    controls.autoRotateSpeed = 0.14
    controls.minPolarAngle = Math.PI * 0.18
    controls.maxPolarAngle = Math.PI * 0.36
    controls.target.set(0, 0.3, 0)

    /* ---- light ---- */
    scene.add(new THREE.HemisphereLight(colors.foreground, colors.background, 0.4))
    const key = new THREE.DirectionalLight(colors.foreground, 1.7)
    key.position.set(-8, 16, 10)
    key.castShadow = true
    key.shadow.mapSize.set(2048, 2048)
    key.shadow.camera.left = -12
    key.shadow.camera.right = 12
    key.shadow.camera.top = 12
    key.shadow.camera.bottom = -12
    key.shadow.camera.near = 1
    key.shadow.camera.far = 50
    key.shadow.bias = -0.0005
    key.shadow.radius = 4
    scene.add(key)
    const rim = new THREE.DirectionalLight(colors.mint, 0.3)
    rim.position.set(10, 6, -14)
    scene.add(rim)

    /* ---- slab and corridor ---- */
    const depth = WING_DEPTH * 2 + CORRIDOR
    const slab = new THREE.Mesh(
      new THREE.BoxGeometry(FLOOR_W + 0.9, 0.18, depth + 0.9),
      new THREE.MeshPhysicalMaterial({ color: colors.background, roughness: 0.45, clearcoat: 0.5, clearcoatRoughness: 0.3 }),
    )
    slab.position.y = -0.1
    slab.receiveShadow = true
    scene.add(slab)
    const corridorFloor = new THREE.Mesh(
      new THREE.PlaneGeometry(FLOOR_W, CORRIDOR),
      new THREE.MeshPhysicalMaterial({ color: colors.elevated.clone().lerp(colors.foreground, 0.08), roughness: 0.3, clearcoat: 0.8, clearcoatRoughness: 0.2 }),
    )
    corridorFloor.rotation.x = -Math.PI / 2
    corridorFloor.position.y = 0.002
    corridorFloor.receiveShadow = true
    scene.add(corridorFloor)
    // A centre line down the corridor: the route the file walks.
    const stripe = new THREE.Mesh(
      new THREE.PlaneGeometry(FLOOR_W - 0.6, 0.05),
      new THREE.MeshBasicMaterial({ color: colors.border.clone().lerp(colors.foreground, 0.25) }),
    )
    stripe.rotation.x = -Math.PI / 2
    stripe.position.y = 0.004
    scene.add(stripe)

    /* ---- rooms ---- */
    const glass = new THREE.MeshPhysicalMaterial({
      color: colors.foreground.clone().lerp(colors.mint, 0.25),
      transparent: true,
      opacity: 0.13,
      roughness: 0.08,
      metalness: 0,
      clearcoat: 1,
      side: THREE.DoubleSide,
      depthWrite: false,
    })
    const frameMaterial = new THREE.MeshPhysicalMaterial({ color: colors.border.clone().lerp(colors.foreground, 0.35), roughness: 0.35, metalness: 0.4 })
    const deskMaterial = new THREE.MeshPhysicalMaterial({ color: colors.elevated.clone().lerp(colors.foreground, 0.28), roughness: 0.4, clearcoat: 0.6 })
    const screenMaterial = new THREE.MeshPhysicalMaterial({ color: colors.background, roughness: 0.2, emissive: colors.mint.clone(), emissiveIntensity: 0.35 })
    const equipmentMaterial = new THREE.MeshPhysicalMaterial({ color: colors.elevated.clone().lerp(colors.foreground, 0.12), roughness: 0.5, metalness: 0.3 })
    const ledMaterial = new THREE.MeshBasicMaterial({ color: colors.mint })

    const wall = (x: number, z: number, w: number, d: number, h = WALL_H) => {
      const mesh = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), glass)
      mesh.position.set(x, h / 2, z)
      scene.add(mesh)
      const top = new THREE.Mesh(new THREE.BoxGeometry(w + 0.02, 0.03, d + 0.02), frameMaterial)
      top.position.set(x, h + 0.015, z)
      scene.add(top)
      const base = new THREE.Mesh(new THREE.BoxGeometry(w + 0.02, 0.06, d + 0.02), frameMaterial)
      base.position.set(x, 0.03, z)
      base.castShadow = true
      scene.add(base)
    }

    const rooms: Room[] = []
    for (const r of layout(departments)) {
      const w = r.x1 - r.x0
      const d = r.z1 - r.z0
      const tone = toneColor(r.dept.tone)
      const tileColor = colors.card.clone().lerp(tone, r.dept.tone === 'advisory' ? 0.06 : 0.1)
      const tile = new THREE.Mesh(
        new THREE.PlaneGeometry(w, d),
        new THREE.MeshPhysicalMaterial({ color: tileColor, roughness: 0.35, clearcoat: 0.7, clearcoatRoughness: 0.25, emissive: tone.clone(), emissiveIntensity: 0 }),
      )
      tile.rotation.x = -Math.PI / 2
      tile.position.set(r.centre.x, 0.003, r.centre.z)
      tile.receiveShadow = true
      scene.add(tile)

      // Three glass walls and a door-side wall with an opening onto the corridor.
      const doorSide = r.dept.wing === 'north' ? r.z1 : r.z0
      const backSide = r.dept.wing === 'north' ? r.z0 : r.z1
      wall(r.centre.x, backSide, w, WALL_T)
      wall(r.x0, r.centre.z, WALL_T, d)
      wall(r.x1, r.centre.z, WALL_T, d)
      const doorW = Math.min(0.9, w * 0.35)
      const sideW = (w - doorW) / 2
      wall(r.x0 + sideW / 2, doorSide, sideW, WALL_T)
      wall(r.x1 - sideW / 2, doorSide, sideW, WALL_T)

      // A skirting strip in the room's tone along the back wall: the door plate.
      const plate = new THREE.Mesh(new THREE.BoxGeometry(Math.min(w * 0.6, 1.6), 0.02, 0.08), new THREE.MeshBasicMaterial({ color: tone }))
      plate.position.set(r.centre.x, 0.012, doorSide + (r.dept.wing === 'north' ? 0.12 : -0.12))
      scene.add(plate)

      const label = document.createElement('div')
      label.className =
        'pointer-events-none absolute -translate-x-1/2 whitespace-nowrap text-center transition-opacity duration-200 [text-shadow:0_1px_3px_rgba(0,0,0,0.9)]'
      // A room named after its one occupant does not repeat the name.
      const solo = r.dept.occupants.length === 1 && r.dept.occupants[0].title === r.dept.title
      const who = r.dept.occupants.length && !solo ? r.dept.occupants.map((o) => o.title).join(' · ') : r.dept.line
      const model = r.dept.occupants[0]?.model ?? ''
      label.innerHTML =
        `<div class="text-[11px] font-semibold tracking-tight" style="color:${tone.getStyle()}">${r.dept.title}</div>` +
        `<div class="text-[10px] text-muted-foreground">${who}</div>` +
        (model ? `<div class="text-[9px] font-mono text-muted-foreground/80">${model}</div>` : '')
      labelHost.appendChild(label)

      const room: Room = { ...r, tile, tileColor, label, figures: [], glow: 0, glowColor: tone.clone() }

      /* ---- furniture and people ---- */
      const facing = r.dept.wing === 'north' ? 1 : -1 // +z looks toward the corridor from the north wing
      const n = r.dept.occupants.length
      if (n > 0) {
        const span = Math.min(w - 0.9, n * 1.15)
        r.dept.occupants.forEach((occupant, i) => {
          const x = n === 1 ? r.centre.x : r.centre.x - span / 2 + (span * i) / (n - 1)
          const z = r.centre.z - facing * 0.25
          const desk = new THREE.Mesh(new THREE.BoxGeometry(0.85, 0.05, 0.42), deskMaterial)
          desk.position.set(x, 0.44, z + facing * 0.5)
          desk.castShadow = true
          desk.receiveShadow = true
          scene.add(desk)
          for (const dx of [-0.36, 0.36]) {
            const leg = new THREE.Mesh(new THREE.BoxGeometry(0.04, 0.42, 0.36), frameMaterial)
            leg.position.set(x + dx, 0.21, z + facing * 0.5)
            scene.add(leg)
          }
          const screen = new THREE.Mesh(new THREE.BoxGeometry(0.34, 0.22, 0.02), screenMaterial)
          screen.position.set(x, 0.6, z + facing * 0.62)
          scene.add(screen)
          const material = new THREE.MeshPhysicalMaterial({
            color: tone,
            metalness: 0.08,
            roughness: 0.3,
            clearcoat: 1,
            clearcoatRoughness: 0.12,
            emissive: tone.clone(),
            emissiveIntensity: 0,
          })
          const group = buildFigure(material)
          group.position.set(x, 0, z)
          scene.add(group)
          room.figures.push({ occupant, group, material, position: group.position.clone(), glow: 0, glowColor: tone.clone() })
        })
      } else {
        // Equipment for the rooms nobody sits in.
        const cz = r.centre.z - facing * 0.3
        if (r.dept.furniture === 'racks') {
          for (let i = 0; i < 3; i++) {
            const rack = new THREE.Mesh(new THREE.BoxGeometry(0.5, 1.0, 0.5), equipmentMaterial)
            rack.position.set(r.centre.x - 0.75 + i * 0.75, 0.5, cz)
            rack.castShadow = true
            scene.add(rack)
            for (let k = 0; k < 4; k++) {
              const led = new THREE.Mesh(new THREE.BoxGeometry(0.06, 0.03, 0.02), ledMaterial)
              led.position.set(rack.position.x - 0.15 + k * 0.1, 0.9, cz + facing * 0.26)
              scene.add(led)
            }
          }
        } else if (r.dept.furniture === 'engine') {
          const core = new THREE.Mesh(new THREE.BoxGeometry(Math.min(1.9, w - 0.8), 0.7, 0.9), equipmentMaterial)
          core.position.set(r.centre.x, 0.35, cz)
          core.castShadow = true
          scene.add(core)
          const strip = new THREE.Mesh(new THREE.BoxGeometry(Math.min(1.7, w - 1.0), 0.05, 0.03), new THREE.MeshBasicMaterial({ color: colors.primary }))
          strip.position.set(r.centre.x, 0.5, cz + facing * 0.46)
          scene.add(strip)
        } else {
          for (let i = 0; i < 2; i++) {
            const shelf = new THREE.Mesh(new THREE.BoxGeometry(Math.min(2.2, w - 0.7), 1.05, 0.3), equipmentMaterial)
            shelf.position.set(r.centre.x, 0.525, r.centre.z - facing * (0.9 - i * 1.1))
            shelf.castShadow = true
            scene.add(shelf)
            for (let k = 0; k < 3; k++) {
              const board = new THREE.Mesh(new THREE.BoxGeometry(Math.min(2.0, w - 0.9), 0.02, 0.26), frameMaterial)
              board.position.set(r.centre.x, 0.25 + k * 0.3, shelf.position.z)
              scene.add(board)
            }
          }
        }
      }
      rooms.push(room)
    }

    const byId = (id: string) => rooms.find((r) => r.dept.id === id)
    const arbiterRoom = rooms.find((r) => r.dept.tone === 'arbiter')
    const vetoRooms = rooms.filter((r) => r.dept.tone === 'veto')
    const advisoryRooms = rooms.filter((r) => r.dept.tone === 'advisory')
    const archive = byId('archive')
    const lab = byId('lab')
    const engine = byId('engine')

    /* ---- the file that walks the floor ---- */
    const glow = glowTexture()
    const fileGroup = new THREE.Group()
    const fileMaterial = new THREE.MeshPhysicalMaterial({ color: colors.foreground, roughness: 0.4, emissive: colors.mint.clone(), emissiveIntensity: 0.25 })
    const sheet = new THREE.Mesh(new THREE.BoxGeometry(0.3, 0.02, 0.4), fileMaterial)
    sheet.castShadow = true
    fileGroup.add(sheet)
    const halo = new THREE.Sprite(new THREE.SpriteMaterial({ map: glow, color: colors.mint, transparent: true, opacity: 0.9, blending: THREE.AdditiveBlending, depthWrite: false }))
    halo.scale.setScalar(0.9)
    fileGroup.add(halo)
    fileGroup.visible = false
    scene.add(fileGroup)

    /* ---- rings, arcs ---- */
    const rings: Ring[] = []
    const ringGeometry = new THREE.RingGeometry(0.3, 0.42, 48)
    const ring = (at: THREE.Vector3, color: THREE.Color, veto: boolean) => {
      const mesh = new THREE.Mesh(
        ringGeometry,
        new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.8, blending: THREE.AdditiveBlending, depthWrite: false, side: THREE.DoubleSide }),
      )
      mesh.rotation.x = -Math.PI / 2
      mesh.position.copy(at).setY(0.014)
      scene.add(mesh)
      rings.push({ mesh, age: 0, life: veto ? 2.8 : 1.1, reach: veto ? 1.3 : 2.4 })
    }

    const arcs: Arc[] = []
    const TAIL_POINTS = 22
    const sendArc = (from: Room, to: Room, color: THREE.Color) => {
      const start = from.centre.clone().setY(1.25)
      const end = to.centre.clone().setY(1.25)
      const mid = start.clone().lerp(end, 0.5)
      mid.y += 1.0 + start.distanceTo(end) * 0.18
      const curve = new THREE.QuadraticBezierCurve3(start, mid, end)
      const head = new THREE.Sprite(new THREE.SpriteMaterial({ map: glow, color, transparent: true, blending: THREE.AdditiveBlending, depthWrite: false }))
      head.scale.setScalar(0.45)
      scene.add(head)
      const tailGeometry = new THREE.BufferGeometry()
      tailGeometry.setAttribute('position', new THREE.Float32BufferAttribute(new Float32Array(TAIL_POINTS * 3), 3))
      const tail = new THREE.Line(tailGeometry, new THREE.LineBasicMaterial({ color, transparent: true, opacity: 0.7, blending: THREE.AdditiveBlending, depthWrite: false }))
      scene.add(tail)
      arcs.push({ curve, t: 0, duration: 0.9 + start.distanceTo(end) * 0.08, head, tail, to, color })
      from.glow = 1
      from.glowColor.copy(color)
      for (const f of from.figures) {
        f.glow = 1
        f.glowColor.copy(color)
      }
    }

    /* ---- the route ---- */
    // Piecewise-linear path: room centre → its door → along the corridor →
    // the next door → the next centre. The file walks it at constant speed.
    const pathBetween = (a: Room, b: Room): THREE.Vector3[] => {
      const y = 0.55
      const corridorA = new THREE.Vector3(a.door.x, y, 0)
      const corridorB = new THREE.Vector3(b.door.x, y, 0)
      return [a.centre.clone().setY(y), a.door.clone().setY(y), corridorA, corridorB, b.door.clone().setY(y), b.centre.clone().setY(y)]
    }

    let route: Stop[] = []
    let stopIndex = 0
    let segment: THREE.Vector3[] = []
    let segmentT = 0
    let segmentLength = 0
    let dwell = 0
    let closed = false
    const SPEED = 2.2

    const planRoute = () => {
      if (!archive || !lab || !engine || !arbiterRoom) return []
      const stops: Stop[] = [
        { room: archive, dwell: 1.0, vetoChance: 0 },
        { room: lab, dwell: 1.6, vetoChance: 0 },
        { room: engine, dwell: 1.8, vetoChance: 0 },
        ...vetoRooms.map((room) => ({ room, dwell: 1.5, vetoChance: 0.22 })),
        { room: arbiterRoom, dwell: 2.2, vetoChance: 0 },
        { room: archive, dwell: 1.2, vetoChance: 0 },
      ]
      return stops
    }

    const startSegment = (from: Room, to: Room) => {
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
      fileGroup.position.copy(route[0].room.centre).setY(0.55)
      halo.material.color.copy(colors.mint)
      fileMaterial.emissive.copy(colors.mint)
      dwell = route[0].dwell
      segment = []
    }

    const arrive = (stop: Stop) => {
      const room = stop.room
      const color = closed ? colors.caution : room.dept.tone === 'arbiter' ? colors.primary : colors.mint
      room.glow = 1
      room.glowColor.copy(color)
      for (const f of room.figures) {
        f.glow = 1
        f.glowColor.copy(color)
      }
      ring(room.centre, color, false)
      if (!closed && stop.vetoChance > 0 && Math.random() < stop.vetoChance) {
        // Stamped. The file turns amber and goes back to the archive as closed.
        closed = true
        ring(room.centre, colors.caution, true)
        room.glowColor.copy(colors.caution)
        for (const f of room.figures) f.glowColor.copy(colors.caution)
        halo.material.color.copy(colors.caution)
        fileMaterial.emissive.copy(colors.caution)
        if (archive) route = [...route.slice(0, stopIndex + 1), { room: archive, dwell: 1.4, vetoChance: 0 }]
      }
      if (room.dept.tone === 'arbiter') {
        // Advisors send their notes while the file is on the desk.
        advisoryRooms.forEach((adv, i) => {
          timers.push(window.setTimeout(() => sendArc(adv, room, colors.mint), 250 + i * 320))
        })
      }
      if (room.dept.tone === 'arbiter' && !closed) {
        ring(room.centre, colors.primary, false)
      }
    }

    const point = (t: number, out: THREE.Vector3) => {
      // Position along the piecewise path at distance t·length.
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
    const timers: number[] = []

    const render = () => {
      const dt = Math.min(clock.getDelta(), 0.05)
      const now = clock.elapsedTime

      if (!reduceMotion) {
        if (!fileGroup.visible) {
          idle -= dt
          if (idle <= 0) beginCycle()
        } else if (segment.length === 0) {
          // Resting on a desk.
          dwell -= dt
          if (dwell <= 0) {
            if (stopIndex + 1 < route.length) {
              startSegment(route[stopIndex].room, route[stopIndex + 1].room)
            } else {
              fileGroup.visible = false
              idle = 1.6
            }
          }
        } else {
          segmentT += (dt * SPEED) / Math.max(segmentLength, 0.001)
          if (segmentT >= 1) {
            stopIndex += 1
            segment = []
            fileGroup.position.copy(route[stopIndex].room.centre).setY(0.55)
            dwell = route[stopIndex].dwell
            arrive(route[stopIndex])
          } else {
            point(segmentT, scratch)
            fileGroup.position.copy(scratch)
          }
        }
        fileGroup.position.y = 0.55 + Math.sin(now * 3) * 0.04
        sheet.rotation.y = now * 0.8

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

      for (const room of rooms) {
        room.glow = Math.max(0, room.glow - dt * 0.9)
        room.tile.material.emissive.copy(room.glowColor)
        room.tile.material.emissiveIntensity = room.glow * 0.22
        for (const f of room.figures) {
          f.glow = Math.max(0, f.glow - dt * 1.2)
          f.material.emissive.copy(f.glowColor)
          f.material.emissiveIntensity = f.glow * 0.8
        }
      }

      controls.update()
      renderer.render(scene, camera)

      const { width, height } = renderer.domElement.getBoundingClientRect()
      for (const room of rooms) {
        projected.copy(room.centre)
        projected.y = WALL_H + 0.55
        projected.project(camera)
        room.label.style.opacity = projected.z > 1 ? '0' : '1'
        room.label.style.left = `${((projected.x + 1) / 2) * width}px`
        room.label.style.top = `${((1 - projected.y) / 2) * height}px`
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
      glow.dispose()
      for (const room of rooms) room.label.remove()
      renderer.dispose()
      renderer.domElement.remove()
    }
  }, [departments])

  return (
    <div
      className={className}
      role="img"
      aria-label="The company as a floor plan: glass-walled rooms on both sides of a corridor. On the north side the arbiter's corner office, the advisory desks and the archive; on the south side the data room, the strategy lab, the engine room and the three veto rooms. A lit file walks the corridor from the archive through the lab and the engine, past each veto room — any of which can stamp it amber and send it back — to the arbiter's office and into the archive."
    >
      <div ref={container} className="absolute inset-0 [&>canvas]:block [&>canvas]:h-full [&>canvas]:w-full" />
      <div ref={labels} className="pointer-events-none absolute inset-0" aria-hidden />
    </div>
  )
}
