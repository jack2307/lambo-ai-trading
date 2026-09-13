/**
 * The floor's models: Kenney's Furniture Kit and Mini Characters (CC0),
 * loaded once and handed out as fitted instances.
 *
 * Nothing on the floor depends on the units a model was made in: an
 * instance is asked for by the size it must be along one axis, measured
 * from its bounding box, then stood on y = 0 and centred in x and z. A
 * character keeps its skeleton through the clone so its clips still play.
 *
 * Loading is best-effort with a deadline. A model that did not arrive is
 * simply absent from the set, and the floor draws its own primitive in
 * that place; the page never waits on the network to show something.
 */

import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { MeshoptDecoder } from 'three/examples/jsm/libs/meshopt_decoder.module.js'
import { clone as cloneSkeleton } from 'three/examples/jsm/utils/SkeletonUtils.js'

interface Loaded {
  scene: THREE.Group
  animations: THREE.AnimationClip[]
  skinned: boolean
}

/** Which axis to size an instance by, and to what. */
export type Fit = { height: number } | { width: number } | { depth: number }

export interface Instance {
  group: THREE.Group
  /** The fitted bounding-box size. */
  size: THREE.Vector3
  animations: THREE.AnimationClip[]
}

export class ModelSet {
  private readonly loaded = new Map<string, Loaded>()

  constructor(entries: [string, Loaded][]) {
    for (const [name, value] of entries) this.loaded.set(name, value)
  }

  has(name: string): boolean {
    return this.loaded.has(name)
  }

  get count(): number {
    return this.loaded.size
  }

  /**
   * A fresh copy of `name`, scaled so the chosen axis measures `fit`,
   * standing on y = 0, centred in x and z, casting and receiving shadows.
   * `null` when the model is not in the set.
   */
  instance(name: string, fit: Fit, tint?: THREE.Color): Instance | null {
    const source = this.loaded.get(name)
    if (!source) return null
    const model = source.skinned ? (cloneSkeleton(source.scene) as THREE.Group) : source.scene.clone(true)
    model.traverse((o) => {
      if (o instanceof THREE.Mesh) {
        o.castShadow = true
        o.receiveShadow = true
        o.frustumCulled = !source.skinned
        if (tint) {
          // A recoloured copy: flat colours take the tint, textures are multiplied by it.
          const materials = Array.isArray(o.material) ? o.material : [o.material]
          const recoloured = materials.map((m) => {
            if (!(m instanceof THREE.MeshStandardMaterial)) return m
            const c = m.clone()
            if (c.map) c.color.copy(tint)
            else c.color.copy(tint)
            return c
          })
          o.material = Array.isArray(o.material) ? recoloured : recoloured[0]
        }
      }
    })
    const box = new THREE.Box3().setFromObject(model)
    const size = box.getSize(new THREE.Vector3())
    const measured = 'height' in fit ? size.y : 'width' in fit ? size.x : size.z
    const target = 'height' in fit ? fit.height : 'width' in fit ? fit.width : fit.depth
    const scale = measured > 0 ? target / measured : 1
    model.scale.setScalar(scale)
    box.setFromObject(model)
    const centre = box.getCenter(new THREE.Vector3())
    model.position.set(-centre.x, -box.min.y, -centre.z)
    const group = new THREE.Group()
    group.add(model)
    return { group, size: box.getSize(new THREE.Vector3()), animations: source.animations }
  }
}

/**
 * Load `names` from `base` (as `<base>/<name>.glb`). Resolves when all have
 * either loaded or failed, or when `deadlineMs` passes — whichever first —
 * with whatever arrived.
 */
export function loadModels(base: string, names: string[], deadlineMs = 8000): Promise<ModelSet> {
  const loader = new GLTFLoader()
  // The packed sets are meshopt-compressed; the decoder is pure JS/wasm inside three.
  loader.setMeshoptDecoder(MeshoptDecoder)
  const entries: [string, Loaded][] = []
  const one = (name: string) =>
    new Promise<void>((resolve) => {
      loader.load(
        `${base}/${name}.glb`,
        (gltf) => {
          let skinned = false
          gltf.scene.traverse((o) => {
            if (o instanceof THREE.SkinnedMesh) skinned = true
            if (o instanceof THREE.Mesh) {
              // Kenney's flat colours are lit for a brighter room than this
              // one: less environment, no metal, a touch darker.
              const materials = Array.isArray(o.material) ? o.material : [o.material]
              for (const m of materials) {
                if (m instanceof THREE.MeshStandardMaterial) {
                  m.envMapIntensity = 0.35
                  m.metalness = 0
                  m.roughness = Math.max(m.roughness, 0.7)
                  if (!m.map) m.color.multiplyScalar(0.82)
                }
              }
            }
          })
          entries.push([name, { scene: gltf.scene, animations: gltf.animations, skinned }])
          resolve()
        },
        undefined,
        (error) => {
          console.warn(`floor model ${name} did not load`, error)
          resolve()
        },
      )
    })
  const all = Promise.all(names.map(one)).then(() => undefined)
  const deadline = new Promise<void>((resolve) => window.setTimeout(resolve, deadlineMs))
  return Promise.race([all, deadline]).then(() => new ModelSet(entries))
}

/** Dispose everything an instance holds: geometry, materials, textures. */
export function disposeDeep(root: THREE.Object3D): void {
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
