/**
 * The review team as chess pieces, in conversation.
 *
 * Chess is the one metaphor whose rules already say what this team's rules
 * say. The pieces are not decoration; each one is chosen for what it can and
 * cannot do:
 *
 * - **The King is the arbiter.** Everything is arranged around it, and it
 *   moves last.
 * - **The Rooks are the veto roles.** A rook cannot be jumped: it blocks a
 *   whole line by itself, which is exactly what a veto does. They stand in a
 *   rank in front of the king — nothing reaches the centre without passing a
 *   rook.
 * - **Bishops and Knights are the advisory roles.** They see a long way and
 *   from unusual angles, and they cannot stop anything alone.
 * - **The board outranks the pieces.** The rules of chess are not up for a
 *   vote, and neither are the gates.
 *
 * The conversation has turns, because a review does. The king asks (lime) and
 * the piece asked answers (mint). Advisory pieces confer among themselves.
 * Now and then a rook interjects (amber), and an amber ring is stamped on the
 * square of whoever it was said to and lingers: that is a "no", and it stays
 * visible longer than anything else on the board on purpose. Each message is
 * an arc of light with a head and a fading tail; the speaker's lacquer glows
 * as it speaks and a ring spreads from its square, and the listener glows on
 * receipt. Underneath, the squares of a rook-shaped path light in sequence,
 * the way a chess interface shows a move.
 *
 * Why this looks like an object: chess pieces are solids of revolution, so a
 * `LatheGeometry` over a smooth profile is the real shape. A lacquered
 * material with an environment to reflect and a shadow to stand in is what
 * turns geometry into a thing on a table.
 *
 * Plain three.js in a ref; every resource disposed once. Labels are DOM,
 * projected. Colours are the CSS tokens, read at mount; the only colours made
 * in code are those tokens lightened or darkened.
 */

import { useEffect, useRef } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js'

export interface Piece {
  id: string
  /** Display name, title case. */
  title: string
  role: 'arbiter' | 'veto' | 'advisory'
  /** The model this role runs on, shown above the name. */
  model: string
}

interface Props {
  pieces: Piece[]
  className?: string
}

const SQUARE = 1
const FILES = 8
const RANKS = 8

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

/** A smooth lathe from a handful of (radius, height) control points. */
function lathe(controls: [number, number][], radial = 56): THREE.LatheGeometry {
  const curve = new THREE.SplineCurve(controls.map(([r, h]) => new THREE.Vector2(r, h)))
  const points = curve.getPoints(90)
  for (const p of points) p.x = Math.max(p.x, 0)
  const geometry = new THREE.LatheGeometry(points, radial)
  geometry.computeVertexNormals()
  return geometry
}

type Kind = 'king' | 'rook' | 'bishop' | 'knight'

/** Build one piece at unit scale, base on y = 0. */
function buildPiece(kind: Kind, material: THREE.Material): THREE.Group {
  const group = new THREE.Group()
  const add = (geometry: THREE.BufferGeometry, y = 0, x = 0, z = 0) => {
    const mesh = new THREE.Mesh(geometry, material)
    mesh.position.set(x, y, z)
    mesh.castShadow = true
    mesh.receiveShadow = true
    group.add(mesh)
    return mesh
  }

  switch (kind) {
    case 'king': {
      add(lathe([[0, 0], [0.44, 0], [0.48, 0.06], [0.45, 0.15], [0.3, 0.26], [0.23, 0.45], [0.19, 0.8], [0.19, 1.05], [0.27, 1.18], [0.34, 1.26], [0.25, 1.36], [0.3, 1.52], [0.2, 1.64], [0.1, 1.7], [0, 1.72]]))
      add(new THREE.BoxGeometry(0.07, 0.34, 0.07), 1.88)
      add(new THREE.BoxGeometry(0.24, 0.07, 0.07), 1.93)
      break
    }
    case 'rook': {
      add(lathe([[0, 0], [0.43, 0], [0.47, 0.06], [0.43, 0.15], [0.3, 0.26], [0.26, 0.5], [0.25, 0.92], [0.33, 1.0], [0.36, 1.08], [0.36, 1.3], [0.27, 1.3], [0.27, 1.2], [0, 1.2]]))
      for (let i = 0; i < 4; i++) {
        const angle = (i / 4) * Math.PI * 2
        const block = add(new THREE.BoxGeometry(0.16, 0.14, 0.11), 1.35, Math.cos(angle) * 0.29, Math.sin(angle) * 0.29)
        block.rotation.y = -angle
      }
      break
    }
    case 'bishop': {
      add(lathe([[0, 0], [0.4, 0], [0.44, 0.06], [0.4, 0.14], [0.27, 0.25], [0.2, 0.5], [0.18, 0.85], [0.27, 0.96], [0.33, 1.12], [0.24, 1.32], [0.1, 1.44], [0, 1.46]]))
      add(new THREE.SphereGeometry(0.075, 24, 24), 1.53)
      break
    }
    case 'knight': {
      add(lathe([[0, 0], [0.4, 0], [0.44, 0.06], [0.4, 0.14], [0.27, 0.25], [0.24, 0.42], [0.27, 0.58], [0, 0.6]]))
      const neck = add(new THREE.CapsuleGeometry(0.19, 0.62, 8, 20), 0.98, -0.02)
      neck.rotation.z = -0.55
      const head = add(new THREE.CapsuleGeometry(0.15, 0.42, 8, 20), 1.34, 0.2)
      head.rotation.z = 1.15
      for (const side of [-1, 1]) {
        const ear = add(new THREE.ConeGeometry(0.06, 0.18, 12), 1.56, 0.02, side * 0.09)
        ear.rotation.z = -0.2
      }
      break
    }
  }
  return group
}

function squareCentre(file: number, rank: number): THREE.Vector3 {
  return new THREE.Vector3((file - (FILES - 1) / 2) * SQUARE, 0, ((RANKS - 1) / 2 - rank) * SQUARE)
}

interface Placed {
  piece: Piece
  kind: Kind
  group: THREE.Group
  material: THREE.MeshPhysicalMaterial
  label: HTMLDivElement
  file: number
  rank: number
  height: number
  /** Emissive glow, 0..1, decays each frame. */
  glow: number
  /** What colour the glow is right now — amber when a rook just spoke to it. */
  glowColor: THREE.Color
}

interface Ring {
  mesh: THREE.Mesh<THREE.RingGeometry, THREE.MeshBasicMaterial>
  age: number
  life: number
  /** Final scale relative to a square. */
  reach: number
}

interface SquareFlash {
  mesh: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>
  age: number
  delay: number
}

interface Message {
  from: Placed
  to: Placed
  color: THREE.Color
  veto: boolean
  curve: THREE.QuadraticBezierCurve3
  t: number
  duration: number
  head: THREE.Sprite
  tail: THREE.Line<THREE.BufferGeometry, THREE.LineBasicMaterial>
  sparks: THREE.Sprite[]
  /** Queue a reply from `to` back to `from` on arrival. */
  reply: boolean
}

export function ChessBoard({ pieces, className }: Props) {
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
    scene.fog = new THREE.Fog(colors.card, 16, 30)

    const pmrem = new THREE.PMREMGenerator(renderer)
    scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture
    scene.environmentIntensity = 0.55
    pmrem.dispose()

    const camera = new THREE.PerspectiveCamera(32, 1, 0.1, 100)
    camera.position.set(5.5, 6.2, 9.5)

    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableZoom = false
    controls.enablePan = false
    controls.enableDamping = true
    controls.dampingFactor = 0.06
    controls.autoRotate = !reduceMotion
    controls.autoRotateSpeed = 0.18
    controls.minPolarAngle = Math.PI * 0.22
    controls.maxPolarAngle = Math.PI * 0.42
    controls.target.set(0, 0.5, -0.3)

    /* ---- light ---- */
    scene.add(new THREE.HemisphereLight(colors.foreground, colors.background, 0.35))
    const key = new THREE.DirectionalLight(colors.foreground, 1.9)
    key.position.set(-5, 11, 6)
    key.castShadow = true
    key.shadow.mapSize.set(2048, 2048)
    key.shadow.camera.left = -7
    key.shadow.camera.right = 7
    key.shadow.camera.top = 7
    key.shadow.camera.bottom = -7
    key.shadow.camera.near = 1
    key.shadow.camera.far = 30
    key.shadow.bias = -0.0006
    key.shadow.radius = 4
    scene.add(key)
    const rim = new THREE.DirectionalLight(colors.mint, 0.35)
    rim.position.set(6, 4, -9)
    scene.add(rim)

    /* ---- board ---- */
    const lightSquare = colors.elevated.clone().lerp(colors.foreground, 0.2)
    const darkSquare = colors.card.clone().lerp(colors.background, 0.4)
    const squareMaterials = {
      light: new THREE.MeshPhysicalMaterial({ color: lightSquare, roughness: 0.32, metalness: 0, clearcoat: 0.7, clearcoatRoughness: 0.25 }),
      dark: new THREE.MeshPhysicalMaterial({ color: darkSquare, roughness: 0.32, metalness: 0, clearcoat: 0.7, clearcoatRoughness: 0.25 }),
    }
    const squareGeometry = new THREE.BoxGeometry(SQUARE, 0.12, SQUARE)
    const board = new THREE.Group()
    for (let file = 0; file < FILES; file++) {
      for (let rank = 0; rank < RANKS; rank++) {
        const square = new THREE.Mesh(squareGeometry, (file + rank) % 2 === 0 ? squareMaterials.dark : squareMaterials.light)
        square.position.copy(squareCentre(file, rank)).setY(-0.06)
        square.receiveShadow = true
        board.add(square)
      }
    }
    const edge = new THREE.Mesh(
      new THREE.BoxGeometry(FILES * SQUARE + 0.7, 0.16, RANKS * SQUARE + 0.7),
      new THREE.MeshPhysicalMaterial({ color: colors.background, roughness: 0.4, clearcoat: 0.5, clearcoatRoughness: 0.3 }),
    )
    edge.position.y = -0.09
    edge.receiveShadow = true
    board.add(edge)
    scene.add(board)

    /* ---- pieces ---- */
    const glow = glowTexture()
    // One material per piece, because each one glows on its own when it
    // speaks. A shared material would light the whole team at once.
    const lacquer = (color: THREE.Color) =>
      new THREE.MeshPhysicalMaterial({
        color,
        metalness: 0.08,
        roughness: 0.28,
        clearcoat: 1,
        clearcoatRoughness: 0.12,
        emissive: color.clone(),
        emissiveIntensity: 0,
      })
    const baseColor = (role: Piece['role']) =>
      role === 'arbiter' ? colors.primary : role === 'veto' ? colors.caution : colors.foreground.clone().lerp(colors.muted, 0.3)
    const heights: Record<Kind, number> = { king: 2.05, rook: 1.45, bishop: 1.6, knight: 1.7 }

    const arbiter = pieces.find((p) => p.role === 'arbiter')
    const vetoes = pieces.filter((p) => p.role === 'veto')
    const advisors = pieces.filter((p) => p.role === 'advisory')

    const placed: Placed[] = []
    const put = (piece: Piece, kind: Kind, file: number, rank: number) => {
      const material = lacquer(baseColor(piece.role))
      const group = buildPiece(kind, material)
      group.position.copy(squareCentre(file, rank))
      group.rotation.y = Math.PI
      scene.add(group)
      const label = document.createElement('div')
      label.className = 'pointer-events-none absolute -translate-x-1/2 whitespace-nowrap text-center transition-opacity duration-200'
      const tint = piece.role === 'arbiter' ? colors.primary : piece.role === 'veto' ? colors.caution : colors.foreground
      const capital = (word: string) => word.charAt(0).toUpperCase() + word.slice(1)
      // Model on top, then the name, then what the piece is and what it may do.
      label.innerHTML =
        `<div class="text-[9px] font-mono text-muted-foreground">${piece.model}</div>` +
        `<div class="text-[11px] font-semibold tracking-tight" style="color:${tint.getStyle()}">${piece.title}</div>` +
        `<div class="text-[10px] text-muted-foreground">${capital(kind)} · ${capital(piece.role)}</div>`
      labelHost.appendChild(label)
      placed.push({ piece, kind, group, material, label, file, rank, height: heights[kind], glow: 0, glowColor: baseColor(piece.role).clone() })
    }

    if (arbiter) put(arbiter, 'king', 4, 1)
    const rookFiles = [2, 4, 6]
    vetoes.slice(0, 3).forEach((piece, i) => put(piece, 'rook', rookFiles[i], 3))
    const advisorySlots: [Kind, number, number][] = [
      ['bishop', 1, 5],
      ['knight', 3, 6],
      ['knight', 5, 6],
      ['bishop', 7, 5],
    ]
    advisors.slice(0, 4).forEach((piece, i) => put(piece, advisorySlots[i][0], advisorySlots[i][1], advisorySlots[i][2]))

    const king = placed.find((p) => p.piece.role === 'arbiter')
    const rooks = placed.filter((p) => p.piece.role === 'veto')
    const minors = placed.filter((p) => p.piece.role === 'advisory')

    /* ---- effects: rings, square flashes, messages ---- */
    const rings: Ring[] = []
    const ringGeometry = new THREE.RingGeometry(0.3, 0.42, 48)
    const ring = (at: Placed, color: THREE.Color, veto: boolean) => {
      const mesh = new THREE.Mesh(
        ringGeometry,
        new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.8, blending: THREE.AdditiveBlending, depthWrite: false, side: THREE.DoubleSide }),
      )
      mesh.rotation.x = -Math.PI / 2
      mesh.position.copy(at.group.position).setY(0.013)
      scene.add(mesh)
      // A veto is stamped, not rippled: it reaches less, lasts longer.
      rings.push({ mesh, age: 0, life: veto ? 2.6 : 1.1, reach: veto ? 1.25 : 2.2 })
    }

    const flashes: SquareFlash[] = []
    const flashGeometry = new THREE.PlaneGeometry(SQUARE * 0.92, SQUARE * 0.92)
    const flashPath = (from: Placed, to: Placed, color: THREE.Color) => {
      let [f, r] = [from.file, from.rank]
      const path: [number, number][] = []
      while (f !== to.file) {
        f += Math.sign(to.file - f)
        path.push([f, r])
      }
      while (r !== to.rank) {
        r += Math.sign(to.rank - r)
        path.push([f, r])
      }
      path.forEach(([pf, pr], i) => {
        const mesh = new THREE.Mesh(
          flashGeometry,
          new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0, blending: THREE.AdditiveBlending, depthWrite: false }),
        )
        mesh.rotation.x = -Math.PI / 2
        mesh.position.copy(squareCentre(pf, pr)).setY(0.011)
        scene.add(mesh)
        flashes.push({ mesh, age: 0, delay: i * 0.07 })
      })
    }

    const messages: Message[] = []
    const pending: { at: number; from: Placed; to: Placed; veto: boolean; reply: boolean }[] = []
    const TAIL_POINTS = 26

    const speak = (from: Placed, to: Placed, veto: boolean, reply: boolean) => {
      const color = veto ? colors.caution : from.piece.role === 'arbiter' ? colors.primary : colors.mint
      const start = from.group.position.clone().setY(from.height + 0.1)
      const end = to.group.position.clone().setY(to.height + 0.1)
      const mid = start.clone().lerp(end, 0.5)
      mid.y += 0.9 + start.distanceTo(end) * 0.28
      const curve = new THREE.QuadraticBezierCurve3(start, mid, end)

      const head = new THREE.Sprite(
        new THREE.SpriteMaterial({ map: glow, color, transparent: true, opacity: 1, blending: THREE.AdditiveBlending, depthWrite: false }),
      )
      head.scale.setScalar(veto ? 0.62 : 0.5)
      scene.add(head)

      const sparks: THREE.Sprite[] = []
      for (let i = 0; i < 3; i++) {
        const spark = new THREE.Sprite(
          new THREE.SpriteMaterial({ map: glow, color, transparent: true, opacity: 0.55 - i * 0.15, blending: THREE.AdditiveBlending, depthWrite: false }),
        )
        spark.scale.setScalar(0.3 - i * 0.06)
        scene.add(spark)
        sparks.push(spark)
      }

      const tailGeometry = new THREE.BufferGeometry()
      tailGeometry.setAttribute('position', new THREE.Float32BufferAttribute(new Float32Array(TAIL_POINTS * 3), 3))
      const tail = new THREE.Line(
        tailGeometry,
        new THREE.LineBasicMaterial({ color, transparent: true, opacity: 0.7, blending: THREE.AdditiveBlending, depthWrite: false }),
      )
      scene.add(tail)

      messages.push({ from, to, color, veto, curve, t: 0, duration: 0.9 + start.distanceTo(end) * 0.12, head, tail, sparks, reply })

      // The speaker lights up and a ring spreads from its square.
      from.glow = 1
      from.glowColor.copy(color)
      ring(from, color, false)
      flashPath(from, to, color)
    }

    /**
     * Who speaks next.
     *
     * Weighted so the board reads as a review rather than static: the king
     * asks and gets answered most of the time, the advisors confer, and a rook
     * cuts in about one exchange in six.
     */
    const nextExchange = () => {
      if (!king || minors.length === 0) return
      const roll = Math.random()
      const pick = <T,>(list: T[]) => list[Math.floor(Math.random() * list.length)]
      if (roll < 0.5) {
        speak(king, pick([...minors, ...rooks]), false, true)
      } else if (roll < 0.82) {
        const a = pick(minors)
        const others = minors.filter((m) => m !== a)
        if (others.length) speak(a, pick(others), false, Math.random() < 0.5)
      } else {
        const rook = pick(rooks)
        if (rook) speak(rook, Math.random() < 0.5 ? king : pick(minors), true, false)
      }
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
    let sinceExchange = 1.2
    const projected = new THREE.Vector3()
    const scratch = new THREE.Vector3()

    const render = () => {
      const dt = Math.min(clock.getDelta(), 0.05)
      const now = clock.elapsedTime

      if (!reduceMotion) {
        sinceExchange += dt
        if (sinceExchange > 2.4 && messages.length < 3) {
          sinceExchange = 0
          nextExchange()
        }
        for (let i = pending.length - 1; i >= 0; i--) {
          if (now >= pending[i].at) {
            const p = pending.splice(i, 1)[0]
            speak(p.from, p.to, p.veto, p.reply)
          }
        }

        // Messages in flight.
        for (let i = messages.length - 1; i >= 0; i--) {
          const m = messages[i]
          m.t += dt / m.duration
          const t = Math.min(m.t, 1)
          // Ease: leaves fast, arrives softly, like something thrown.
          const eased = 1 - Math.pow(1 - t, 2.2)
          m.curve.getPointAt(eased, scratch)
          m.head.position.copy(scratch)
          for (let s = 0; s < m.sparks.length; s++) {
            m.curve.getPointAt(Math.max(0, eased - 0.05 * (s + 1)), scratch)
            m.sparks[s].position.copy(scratch)
          }
          const positions = m.tail.geometry.getAttribute('position') as THREE.BufferAttribute
          const tailStart = Math.max(0, eased - 0.32)
          for (let p = 0; p < TAIL_POINTS; p++) {
            m.curve.getPointAt(tailStart + ((eased - tailStart) * p) / (TAIL_POINTS - 1), scratch)
            positions.setXYZ(p, scratch.x, scratch.y, scratch.z)
          }
          positions.needsUpdate = true

          if (m.t >= 1) {
            // Arrival: the listener glows in the message's colour. A veto
            // leaves its mark on the listener's square and lingers.
            m.to.glow = 1
            m.to.glowColor.copy(m.color)
            ring(m.to, m.color, m.veto)
            if (m.reply) pending.push({ at: now + 0.35, from: m.to, to: m.from, veto: false, reply: false })
            scene.remove(m.head, m.tail, ...m.sparks)
            m.head.material.dispose()
            m.tail.geometry.dispose()
            m.tail.material.dispose()
            for (const s of m.sparks) s.material.dispose()
            messages.splice(i, 1)
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

        for (let i = flashes.length - 1; i >= 0; i--) {
          const f = flashes[i]
          f.age += dt
          const t = Math.max(0, f.age - f.delay) / 0.85
          const envelope = t < 0.15 ? t / 0.15 : Math.max(0, 1 - (t - 0.15) / 0.85)
          f.mesh.material.opacity = envelope * 0.32
          if (t >= 1) {
            scene.remove(f.mesh)
            f.mesh.material.dispose()
            flashes.splice(i, 1)
          }
        }
      }

      // Piece glow decays whatever the motion setting; it only ever rises
      // from a message, and with motion off none are sent.
      for (const entry of placed) {
        entry.glow = Math.max(0, entry.glow - dt * 1.4)
        entry.material.emissive.copy(entry.glowColor)
        entry.material.emissiveIntensity = entry.glow * 0.85
      }

      controls.update()
      renderer.render(scene, camera)

      const { width, height } = renderer.domElement.getBoundingClientRect()
      for (const entry of placed) {
        entry.group.getWorldPosition(projected)
        projected.y += entry.height + 0.4
        projected.project(camera)
        entry.label.style.opacity = projected.z > 1 ? '0' : '1'
        entry.label.style.left = `${((projected.x + 1) / 2) * width}px`
        entry.label.style.top = `${((1 - projected.y) / 2) * height}px`
      }
      frame = requestAnimationFrame(render)
    }
    frame = requestAnimationFrame(render)

    return () => {
      cancelAnimationFrame(frame)
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
      squareGeometry.dispose()
      ringGeometry.dispose()
      flashGeometry.dispose()
      glow.dispose()
      for (const entry of placed) entry.label.remove()
      renderer.dispose()
      renderer.domElement.remove()
    }
  }, [pieces])

  return (
    <div
      className={className}
      role="img"
      aria-label="The review team as chess pieces in conversation. The king is the arbiter on the back rank; three rooks — the roles that can veto alone — stand in a rank in front of it. Bishops and knights on the wings are the advisory roles. Arcs of light carry messages between pieces: the king asks in lime and is answered in mint, and when a rook interjects in amber, an amber ring is stamped on the listener's square and lingers."
    >
      <div ref={container} className="absolute inset-0 [&>canvas]:block [&>canvas]:h-full [&>canvas]:w-full" />
      <div ref={labels} className="pointer-events-none absolute inset-0" aria-hidden />
    </div>
  )
}
