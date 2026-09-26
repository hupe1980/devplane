//#region \0vite/modulepreload-polyfill.js
(function polyfill() {
	const relList = document.createElement("link").relList;
	if (relList && relList.supports && relList.supports("modulepreload")) return;
	for (const link of document.querySelectorAll("link[rel=\"modulepreload\"]")) processPreload(link);
	new MutationObserver((mutations) => {
		for (const mutation of mutations) {
			if (mutation.type !== "childList") continue;
			for (const node of mutation.addedNodes) if (node.tagName === "LINK" && node.rel === "modulepreload") processPreload(node);
		}
	}).observe(document, {
		childList: true,
		subtree: true
	});
	function getFetchOpts(link) {
		const fetchOpts = {};
		if (link.integrity) fetchOpts.integrity = link.integrity;
		if (link.referrerPolicy) fetchOpts.referrerPolicy = link.referrerPolicy;
		if (link.crossOrigin === "use-credentials") fetchOpts.credentials = "include";
		else if (link.crossOrigin === "anonymous") fetchOpts.credentials = "omit";
		else fetchOpts.credentials = "same-origin";
		return fetchOpts;
	}
	function processPreload(link) {
		if (link.ep) return;
		link.ep = true;
		const fetchOpts = getFetchOpts(link);
		fetch(link.href, fetchOpts);
	}
})();
//#endregion
//#region node_modules/svelte/src/internal/shared/utils.js
var is_array = Array.isArray;
var index_of = Array.prototype.indexOf;
var includes = Array.prototype.includes;
var array_from = Array.from;
var define_property = Object.defineProperty;
var get_descriptor = Object.getOwnPropertyDescriptor;
var get_descriptors = Object.getOwnPropertyDescriptors;
var object_prototype = Object.prototype;
var array_prototype = Array.prototype;
var get_prototype_of = Object.getPrototypeOf;
var is_extensible = Object.isExtensible;
/**
* @param {any} thing
* @returns {thing is Function}
*/
function is_function(thing) {
	return typeof thing === "function";
}
var noop = () => {};
/** @param {Array<() => void>} arr */
function run_all(arr) {
	for (var i = 0; i < arr.length; i++) arr[i]();
}
/**
* TODO replace with Promise.withResolvers once supported widely enough
* @template [T=void]
*/
function deferred() {
	/** @type {(value: T) => void} */
	var resolve;
	/** @type {(reason: any) => void} */
	var reject;
	return {
		promise: new Promise((res, rej) => {
			resolve = res;
			reject = rej;
		}),
		resolve,
		reject
	};
}
/**
* When encountering a situation like `let [a, b, c] = $derived(blah())`,
* we need to stash an intermediate value that `a`, `b`, and `c` derive
* from, in case it's an iterable
* @template T
* @param {ArrayLike<T> | Iterable<T>} value
* @param {number} [n]
* @returns {Array<T>}
*/
function to_array(value, n) {
	if (Array.isArray(value)) return value;
	if (n === void 0 || !(Symbol.iterator in value)) return Array.from(value);
	/** @type {T[]} */
	const array = [];
	for (const element of value) {
		array.push(element);
		if (array.length === n) break;
	}
	return array;
}
var CLEAN = 1024;
var DIRTY = 2048;
var MAYBE_DIRTY = 4096;
var INERT = 8192;
var DESTROYED = 16384;
/** Set once a reaction has run for the first time */
var REACTION_RAN = 32768;
/** Effect is in the process of getting destroyed. Can be observed in child teardown functions */
var DESTROYING = 1 << 25;
/**
* 'Transparent' effects do not create a transition boundary.
* This is on a block effect 99% of the time but may also be on a branch effect if its parent block effect was pruned
*/
var EFFECT_TRANSPARENT = 65536;
var EFFECT_PRESERVED = 1 << 19;
var USER_EFFECT = 1 << 20;
var EFFECT_OFFSCREEN = 1 << 25;
var REACTION_IS_UPDATING = 1 << 21;
var ASYNC = 1 << 22;
var ERROR_VALUE = 1 << 23;
var STATE_SYMBOL = Symbol("$state");
/** Marks component export objects, so that `proxy(...)` leaves them untouched */
var COMPONENT_SYMBOL = Symbol("component");
var LEGACY_PROPS = Symbol("legacy props");
var LOADING_ATTR_SYMBOL = Symbol("");
var ATTRIBUTES_CACHE = Symbol("attributes");
var CLASS_CACHE = Symbol("class");
var STYLE_CACHE = Symbol("style");
var TEXT_CACHE = Symbol("text");
var FORM_RESET_HANDLER = Symbol("form reset");
/** allow users to ignore aborted signal errors if `reason.name === 'StaleReactionError` */
var STALE_REACTION = new class StaleReactionError extends Error {
	name = "StaleReactionError";
	message = "The reaction that called `getAbortSignal()` was re-run or destroyed";
}();
var IS_XHTML = !!globalThis.document?.contentType && /* @__PURE__ */ globalThis.document.contentType.includes("xml");
//#endregion
//#region node_modules/svelte/src/constants.js
var HYDRATION_ERROR = {};
var UNINITIALIZED = Symbol("uninitialized");
var NAMESPACE_HTML = "http://www.w3.org/1999/xhtml";
/**
* Reading a derived belonging to a now-destroyed effect may result in stale values
*/
function derived_inert() {
	console.warn(`https://svelte.dev/e/derived_inert`);
}
/**
* Hydration failed because the initial UI does not match what was rendered on the server. The error occurred near %location%
* @param {string | undefined | null} [location]
*/
function hydration_mismatch(location) {
	console.warn(`https://svelte.dev/e/hydration_mismatch`);
}
/**
* The `value` property of a `<select multiple>` element should be an array, but it received a non-array value. The selection will be kept as is.
*/
function select_multiple_invalid_value() {
	console.warn(`https://svelte.dev/e/select_multiple_invalid_value`);
}
/**
* A `<svelte:boundary>` `reset` function only resets the boundary the first time it is called
*/
function svelte_boundary_reset_noop() {
	console.warn(`https://svelte.dev/e/svelte_boundary_reset_noop`);
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/hydration.js
/** @import { TemplateNode } from '#client' */
/**
* Use this variable to guard everything related to hydration code so it can be treeshaken out
* if the user doesn't use the `hydrate` method and these code paths are therefore not needed.
*/
var hydrating = false;
/** @param {boolean} value */
function set_hydrating(value) {
	hydrating = value;
}
/**
* The node that is currently being hydrated. This starts out as the first node inside the opening
* <!--[--> comment, and updates each time a component calls `$.child(...)` or `$.sibling(...)`.
* When entering a block (e.g. `{#if ...}`), `hydrate_node` is the block opening comment; by the
* time we leave the block it is the closing comment, which serves as the block's anchor.
* @type {TemplateNode}
*/
var hydrate_node;
/** @param {TemplateNode | null} node */
function set_hydrate_node(node) {
	if (node === null) {
		hydration_mismatch();
		throw HYDRATION_ERROR;
	}
	return hydrate_node = node;
}
function hydrate_next() {
	return set_hydrate_node(/* @__PURE__ */ get_next_sibling(hydrate_node));
}
/** @param {TemplateNode} node */
function reset(node) {
	if (!hydrating) return;
	if (/* @__PURE__ */ get_next_sibling(hydrate_node) !== null) {
		hydration_mismatch();
		throw HYDRATION_ERROR;
	}
	hydrate_node = node;
}
function next(count = 1) {
	if (hydrating) {
		var i = count;
		var node = hydrate_node;
		while (i--) node = /* @__PURE__ */ get_next_sibling(node);
		hydrate_node = node;
	}
}
/**
* Skips or removes (depending on {@link remove}) all nodes starting at `hydrate_node` up until the next hydration end comment
* @param {boolean} remove
*/
function skip_nodes(remove = true) {
	var depth = 0;
	var node = hydrate_node;
	while (true) {
		if (node.nodeType === 8) {
			var data = node.data;
			if (data === "]") {
				if (depth === 0) return node;
				depth -= 1;
			} else if (data === "[" || data === "[!" || data[0] === "[" && !isNaN(Number(data.slice(1)))) depth += 1;
		}
		var next = /* @__PURE__ */ get_next_sibling(node);
		if (remove) node.remove();
		node = next;
	}
}
/**
*
* @param {TemplateNode} node
*/
function read_hydration_instruction(node) {
	if (!node || node.nodeType !== 8) {
		hydration_mismatch();
		throw HYDRATION_ERROR;
	}
	return node.data;
}
//#endregion
//#region node_modules/svelte/src/internal/client/reactivity/equality.js
/** @import { Equals } from '#client' */
/** @type {Equals} */
function equals(value) {
	return value === this.v;
}
/**
* @param {unknown} a
* @param {unknown} b
* @returns {boolean}
*/
function safe_not_equal(a, b) {
	return a != a ? b == b : a !== b || a !== null && typeof a === "object" || typeof a === "function";
}
/** @type {Equals} */
function safe_equals(value) {
	return !safe_not_equal(value, this.v);
}
//#endregion
//#region node_modules/svelte/src/internal/client/errors.js
/**
* Cannot create a `$derived(...)` with an `await` expression outside of an effect tree
* @returns {never}
*/
function async_derived_orphan() {
	throw new Error(`https://svelte.dev/e/async_derived_orphan`);
}
/**
* Keyed each block has duplicate key `%value%` at indexes %a% and %b%
* @param {string} a
* @param {string} b
* @param {string | undefined | null} [value]
* @returns {never}
*/
function each_key_duplicate(a, b, value) {
	throw new Error(`https://svelte.dev/e/each_key_duplicate`);
}
/**
* `%rune%` cannot be used inside an effect cleanup function
* @param {string} rune
* @returns {never}
*/
function effect_in_teardown(rune) {
	throw new Error(`https://svelte.dev/e/effect_in_teardown`);
}
/**
* Effect cannot be created inside a `$derived` value that was not itself created inside an effect
* @returns {never}
*/
function effect_in_unowned_derived() {
	throw new Error(`https://svelte.dev/e/effect_in_unowned_derived`);
}
/**
* `%rune%` can only be used inside an effect (e.g. during component initialisation)
* @param {string} rune
* @returns {never}
*/
function effect_orphan(rune) {
	throw new Error(`https://svelte.dev/e/effect_orphan`);
}
/**
* Maximum update depth exceeded. This typically indicates that an effect reads and writes the same piece of state
* @returns {never}
*/
function effect_update_depth_exceeded() {
	throw new Error(`https://svelte.dev/e/effect_update_depth_exceeded`);
}
/**
* Cannot do `bind:%key%={undefined}` when `%key%` has a fallback value
* @param {string} key
* @returns {never}
*/
function props_invalid_value(key) {
	throw new Error(`https://svelte.dev/e/props_invalid_value`);
}
/**
* Property descriptors defined on `$state` objects must contain `value` and always be `enumerable`, `configurable` and `writable`.
* @returns {never}
*/
function state_descriptors_fixed() {
	throw new Error(`https://svelte.dev/e/state_descriptors_fixed`);
}
/**
* Cannot set prototype of `$state` object
* @returns {never}
*/
function state_prototype_fixed() {
	throw new Error(`https://svelte.dev/e/state_prototype_fixed`);
}
/**
* Updating state inside `$derived(...)`, `$inspect(...)` or a template expression is forbidden. If the value should not be reactive, declare it without `$state`
* @returns {never}
*/
function state_unsafe_mutation() {
	throw new Error(`https://svelte.dev/e/state_unsafe_mutation`);
}
/**
* A `<svelte:boundary>` `reset` function cannot be called while an error is still being handled
* @returns {never}
*/
function svelte_boundary_reset_onerror() {
	throw new Error(`https://svelte.dev/e/svelte_boundary_reset_onerror`);
}
//#endregion
//#region node_modules/svelte/src/internal/flags/index.js
/** True if experimental.async=true */
var async_mode_flag = false;
/** True if we're not certain that we only have Svelte 5 code in the compilation */
var legacy_mode_flag = false;
//#endregion
//#region node_modules/svelte/src/internal/shared/clone.js
/** @import { Snapshot } from './types' */
/**
* In dev, we keep track of which properties could not be cloned. In prod
* we don't bother, but we keep a dummy array around so that the
* signature stays the same
* @type {string[]}
*/
var empty = [];
/**
* @template T
* @param {T} value
* @param {boolean} [skip_warning]
* @param {boolean} [no_tojson]
* @returns {Snapshot<T>}
*/
function snapshot(value, skip_warning = false, no_tojson = false) {
	return clone(value, /* @__PURE__ */ new Map(), "", empty, null, no_tojson);
}
/**
* @template T
* @param {T} value
* @param {Map<T, Snapshot<T>>} cloned
* @param {string} path
* @param {string[]} paths
* @param {null | T} [original] The original value, if `value` was produced from a `toJSON` call
* @param {boolean} [no_tojson]
* @returns {Snapshot<T>}
*/
function clone(value, cloned, path, paths, original = null, no_tojson = false) {
	if (typeof value === "object" && value !== null) {
		var unwrapped = cloned.get(value);
		if (unwrapped !== void 0) return unwrapped;
		if (value instanceof Map) return new Map(value);
		if (value instanceof Set) return new Set(value);
		if (is_array(value)) {
			var copy = Array(value.length);
			cloned.set(value, copy);
			if (original !== null) cloned.set(original, copy);
			for (var i = 0; i < value.length; i += 1) {
				var element = value[i];
				if (i in value) copy[i] = clone(element, cloned, path, paths, null, no_tojson);
			}
			return copy;
		}
		if (get_prototype_of(value) === object_prototype) {
			/** @type {Snapshot<any>} */
			copy = {};
			cloned.set(value, copy);
			if (original !== null) cloned.set(original, copy);
			for (var key of Object.keys(value)) copy[key] = clone(value[key], cloned, path, paths, null, no_tojson);
			return copy;
		}
		if (value instanceof Date) {
			value.getTime();
			return structuredClone(value);
		}
		if (typeof value.toJSON === "function" && !no_tojson) return clone(
			/** @type {T & { toJSON(): any } } */
			value.toJSON(),
			cloned,
			path,
			paths,
			value
		);
	}
	if (value instanceof EventTarget) return value;
	try {
		return structuredClone(value);
	} catch (e) {
		return value;
	}
}
//#endregion
//#region node_modules/svelte/src/internal/client/context.js
/** @import { ComponentContext, DevStackEntry, Effect } from '#client' */
/** @type {ComponentContext | null} */
var component_context = null;
/** @param {ComponentContext | null} context */
function set_component_context(context) {
	component_context = context;
}
/**
* @param {Record<string, unknown>} props
* @param {any} runes
* @param {Function} [fn]
* @returns {void}
*/
function push(props, runes = false, fn) {
	component_context = {
		p: component_context,
		i: false,
		c: null,
		e: null,
		s: props,
		x: null,
		r: active_effect,
		l: legacy_mode_flag && !runes ? {
			s: null,
			u: null,
			$: []
		} : null
	};
}
/**
* @template {Record<string, any>} T
* @param {T} [component]
* @returns {T}
*/
function pop(component) {
	var context = component_context;
	var effects = context.e;
	if (effects !== null) {
		context.e = null;
		for (var fn of effects) create_user_effect(fn);
	}
	if (component !== void 0) context.x = component;
	context.i = true;
	component_context = context.p;
	return mark_as_component(component);
}
/**
* Add a symbol to the object (or create one if undefined) to mark it as a component so it isn't proxified.
* @param {any} component
*/
function mark_as_component(component = {}) {
	define_property(component, COMPONENT_SYMBOL, { value: true });
	return component;
}
/** @returns {boolean} */
function is_runes() {
	return !legacy_mode_flag || component_context !== null && component_context.l === null;
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/task.js
/** @type {Array<() => void>} */
var micro_tasks = [];
function run_micro_tasks() {
	var tasks = micro_tasks;
	micro_tasks = [];
	run_all(tasks);
}
/**
* @param {() => void} fn
*/
function queue_micro_task(fn) {
	if (micro_tasks.length === 0 && !is_flushing_sync) {
		var tasks = micro_tasks;
		queueMicrotask(() => {
			if (tasks === micro_tasks) run_micro_tasks();
		});
	}
	micro_tasks.push(fn);
}
/**
* Synchronously run any queued tasks.
*/
function flush_tasks() {
	while (micro_tasks.length > 0) run_micro_tasks();
}
//#endregion
//#region node_modules/svelte/src/internal/client/reactivity/status.js
/** @import { Derived, Signal } from '#client' */
var STATUS_MASK = ~(DIRTY | MAYBE_DIRTY | CLEAN);
/**
* @param {Signal} signal
* @param {number} status
*/
function set_signal_status(signal, status) {
	signal.f = signal.f & STATUS_MASK | status;
}
/**
* Set a derived's status to CLEAN or MAYBE_DIRTY based on its connection state.
* @param {Derived} derived
*/
function update_derived_status(derived) {
	if ((derived.f & 512) !== 0 || derived.deps === null) set_signal_status(derived, CLEAN);
	else set_signal_status(derived, MAYBE_DIRTY);
}
//#endregion
//#region node_modules/svelte/src/internal/client/reactivity/utils.js
/** @import { Effect } from '#client' */
/**
* @param {Effect} effect
* @param {Set<Effect>} dirty_effects
* @param {Set<Effect>} maybe_dirty_effects
*/
function defer_effect(effect, dirty_effects, maybe_dirty_effects) {
	if ((effect.f & 2048) !== 0) dirty_effects.add(effect);
	else if ((effect.f & 4096) !== 0) maybe_dirty_effects.add(effect);
	set_signal_status(effect, CLEAN);
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/elements/misc.js
/**
* @param {HTMLElement} dom
* @param {boolean} value
* @returns {void}
*/
function autofocus(dom, value) {
	if (value) {
		const body = document.body;
		dom.autofocus = true;
		queue_micro_task(() => {
			if (document.activeElement === body) dom.focus();
		});
	}
}
/**
* The child of a textarea actually corresponds to the defaultValue property, so we need
* to remove it upon hydration to avoid a bug when someone resets the form value.
* @param {HTMLTextAreaElement} dom
* @returns {void}
*/
function remove_textarea_child(dom) {
	if (hydrating && /* @__PURE__ */ get_first_child(dom) !== null) clear_text_content(dom);
}
var listening_to_form_reset = false;
function add_form_reset_listener() {
	if (!listening_to_form_reset) {
		listening_to_form_reset = true;
		document.addEventListener("reset", (evt) => {
			Promise.resolve().then(() => {
				if (!evt.defaultPrevented) for (const e of evt.target.elements)
 /** @type {any} */ e[FORM_RESET_HANDLER]?.();
			});
		}, { capture: true });
	}
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/elements/bindings/shared.js
/**
* @template T
* @param {() => T} fn
*/
function without_reactive_context(fn) {
	var previous_reaction = active_reaction;
	var previous_effect = active_effect;
	set_active_reaction(null);
	set_active_effect(null);
	try {
		return fn();
	} finally {
		set_active_reaction(previous_reaction);
		set_active_effect(previous_effect);
	}
}
/**
* Listen to the given event, and then instantiate a global form reset listener if not already done,
* to notify all bindings when the form is reset
* @param {HTMLElement} element
* @param {string} event
* @param {(is_reset?: true) => void} handler
* @param {(is_reset?: true) => void} [on_reset]
*/
function listen_to_event_and_reset_event(element, event, handler, on_reset = handler) {
	element.addEventListener(event, () => without_reactive_context(handler));
	const prev = element[FORM_RESET_HANDLER];
	if (prev)
 /** @type {any} */ element[FORM_RESET_HANDLER] = () => {
		prev();
		on_reset(true);
	};
	else
 /** @type {any} */ element[FORM_RESET_HANDLER] = () => on_reset(true);
	add_form_reset_listener();
}
//#endregion
//#region node_modules/svelte/src/internal/client/reactivity/async.js
/** @import { Blocker, Effect, Source, Value } from '#client' */
/**
* @param {Blocker[]} blockers
* @param {Array<() => any>} sync
* @param {Array<() => Promise<any>>} async
* @param {(values: Value[]) => any} fn
*/
function flatten(blockers, sync, async, fn) {
	const d = is_runes() ? derived : derived_safe_equal;
	var pending = blockers.filter((b) => !b.settled);
	var deriveds = sync.map(d);
	if (async.length === 0 && pending.length === 0) {
		fn(deriveds);
		return;
	}
	var parent = active_effect;
	var restore = capture();
	var blocker_promise = pending.length === 1 ? pending[0].promise : pending.length > 1 ? Promise.all(pending.map((b) => b.promise)) : null;
	/**
	* @param {Source[]} async
	*/
	function finish(async) {
		if ((parent.f & 16384) !== 0) return;
		restore();
		try {
			fn([...deriveds, ...async]);
		} catch (error) {
			invoke_error_boundary(error, parent);
		}
		unset_context();
	}
	var decrement_pending = increment_pending();
	if (async.length === 0) {
		/** @type {Promise<any>} */ blocker_promise.then(() => finish([])).finally(decrement_pending);
		return;
	}
	function run() {
		Promise.all(async.map((expression) => /* @__PURE__ */ async_derived(expression))).then(finish).catch((error) => invoke_error_boundary(error, parent)).finally(decrement_pending);
	}
	if (blocker_promise) blocker_promise.then(() => {
		restore();
		run();
		unset_context();
	});
	else run();
}
/**
* Captures the current effect context so that we can restore it after
* some asynchronous work has happened (so that e.g. `await a + b`
* causes `b` to be registered as a dependency).
*/
function capture() {
	var previous_effect = active_effect;
	var previous_reaction = active_reaction;
	var previous_component_context = component_context;
	var previous_batch = current_batch;
	return function restore(activate_batch = true) {
		set_active_effect(previous_effect);
		set_active_reaction(previous_reaction);
		set_component_context(previous_component_context);
		if (activate_batch && (previous_effect.f & 16384) === 0) {
			previous_batch?.activate();
			previous_batch?.apply();
		}
	};
}
function unset_context(deactivate_batch = true) {
	set_active_effect(null);
	set_active_reaction(null);
	set_component_context(null);
	if (deactivate_batch) current_batch?.deactivate();
}
/**
* @returns {(skip?: boolean) => void}
*/
function increment_pending() {
	var effect = active_effect;
	var boundary = effect.b;
	var batch = current_batch;
	var blocking = !!boundary?.is_rendered();
	boundary?.update_pending_count(1, batch);
	batch.increment(blocking, effect);
	return () => {
		boundary?.update_pending_count(-1, batch);
		batch.decrement(blocking, effect);
	};
}
/**
* @template V
* @param {() => V} fn
* @returns {Derived<V>}
*/
/*#__NO_SIDE_EFFECTS__*/
function derived(fn) {
	var flags = 2 | DIRTY;
	if (active_effect !== null) active_effect.f |= EFFECT_PRESERVED;
	return {
		ctx: component_context,
		deps: null,
		effects: null,
		equals,
		f: flags,
		fn,
		reactions: null,
		rv: 0,
		v: UNINITIALIZED,
		wv: 0,
		parent: active_effect,
		ac: null
	};
}
var OBSOLETE = Symbol("obsolete");
/**
* @template V
* @param {() => V | Promise<V>} fn
* @param {string} [label]
* @param {string} [location] If provided, print a warning if the value is not read immediately after update
* @returns {Promise<Source<V>>}
*/
/*#__NO_SIDE_EFFECTS__*/
function async_derived(fn, label, location) {
	let parent = active_effect;
	if (parent === null) async_derived_orphan();
	var promise = void 0;
	var signal = source(UNINITIALIZED);
	var should_suspend = !active_reaction;
	/** @type {Set<ReturnType<typeof deferred<V>>>} */
	var deferreds = /* @__PURE__ */ new Set();
	async_effect(() => {
		var effect = active_effect;
		/** @type {ReturnType<typeof deferred<V>>} */
		var d = deferred();
		promise = d.promise;
		try {
			Promise.resolve(fn()).then(d.resolve, (e) => {
				if (e !== STALE_REACTION) d.reject(e);
			}).finally(unset_context);
		} catch (error) {
			d.reject(error);
			unset_context();
		}
		var batch = current_batch;
		if (should_suspend) {
			if ((effect.f & 32768) !== 0) var decrement_pending = increment_pending();
			if (parent.b?.is_rendered()) batch.async_deriveds.get(effect)?.reject(OBSOLETE);
			else for (const d of deferreds.values()) d.reject(OBSOLETE);
			deferreds.add(d);
			batch.async_deriveds.set(effect, d);
		}
		/**
		* @param {any} value
		* @param {unknown} error
		*/
		const handler = (value, error = void 0) => {
			decrement_pending?.();
			deferreds.delete(d);
			if (error === OBSOLETE) return;
			batch.activate();
			if (error) {
				signal.f |= ERROR_VALUE;
				internal_set(signal, error);
			} else {
				if ((signal.f & 8388608) !== 0) signal.f ^= ERROR_VALUE;
				internal_set(signal, value);
			}
			batch.deactivate();
		};
		d.promise.then(handler, (e) => handler(null, e || "unknown"));
	});
	teardown(() => {
		for (const d of deferreds) d.reject(OBSOLETE);
	});
	return new Promise((fulfil) => {
		/** @param {Promise<V>} p */
		function next(p) {
			function go() {
				if (p === promise) fulfil(signal);
				else next(promise);
			}
			p.then(go, go);
		}
		next(promise);
	});
}
/**
* @template V
* @param {() => V} fn
* @returns {Derived<V>}
*/
/*#__NO_SIDE_EFFECTS__*/
function user_derived(fn) {
	const d = /* @__PURE__ */ derived(fn);
	if (!async_mode_flag) push_reaction_value(d);
	return d;
}
/**
* @template V
* @param {() => V} fn
* @returns {Derived<V>}
*/
/*#__NO_SIDE_EFFECTS__*/
function derived_safe_equal(fn) {
	const signal = /* @__PURE__ */ derived(fn);
	signal.equals = safe_equals;
	return signal;
}
/**
* @param {Derived} derived
* @returns {void}
*/
function destroy_derived_effects(derived) {
	var effects = derived.effects;
	if (effects !== null) {
		derived.effects = null;
		for (var i = 0; i < effects.length; i += 1) destroy_effect(effects[i]);
	}
}
/**
* @template T
* @param {Derived} derived
* @returns {T}
*/
function execute_derived(derived) {
	var value;
	var prev_active_effect = active_effect;
	var parent = derived.parent;
	if (!is_destroying_effect && parent !== null && derived.v !== UNINITIALIZED && (parent.f & 24576) !== 0) {
		derived_inert();
		return derived.v;
	}
	set_active_effect(parent);
	try {
		destroy_derived_effects(derived);
		value = update_reaction(derived);
	} finally {
		set_active_effect(prev_active_effect);
	}
	return value;
}
/**
* @param {Derived} derived
* @returns {void}
*/
function update_derived(derived) {
	var value = execute_derived(derived);
	if (!derived.equals(value)) {
		derived.wv = increment_write_version();
		if (!current_batch?.is_fork || derived.deps === null) {
			if (current_batch !== null) {
				current_batch.capture(derived, value, true);
				previous_batch?.capture(derived, value, true);
			} else derived.v = value;
			if (derived.deps === null) {
				set_signal_status(derived, CLEAN);
				return;
			}
		}
	}
	if (is_destroying_effect) return;
	if (batch_values !== null) {
		if (effect_tracking() || current_batch?.is_fork) batch_values.set(derived, value);
	} else update_derived_status(derived);
}
/**
* @param {Derived} derived
*/
function freeze_derived_effects(derived) {
	if (derived.effects === null) return;
	for (const e of derived.effects) if (e.teardown || e.ac) {
		e.teardown?.();
		if (e.ac !== null) without_reactive_context(() => {
			/** @type {AbortController} */ e.ac.abort(STALE_REACTION);
			e.ac = null;
		});
		if (e.fn !== null) e.teardown = noop;
		remove_reactions(e, 0);
		destroy_effect_children(e);
	}
}
/**
* @param {Derived} derived
*/
function unfreeze_derived_effects(derived) {
	if (derived.effects === null) return;
	for (const e of derived.effects) if (e.teardown && e.fn !== null) update_effect(e);
}
//#endregion
//#region node_modules/svelte/src/internal/client/reactivity/batch.js
/** @import { Fork } from 'svelte' */
/** @import { Derived, Effect, Reaction, Source, Value } from '#client' */
/** @type {Batch | null} */
var first_batch = null;
/** @type {Batch | null} */
var last_batch = null;
/** @type {Batch | null} */
var current_batch = null;
/**
* This is needed to avoid overwriting inputs
* @type {Batch | null}
*/
var previous_batch = null;
/**
* When time travelling (i.e. working in one batch, while other batches
* still have ongoing work), we ignore the real values of affected
* signals in favour of their values within the batch
* @type {Map<Value, any> | null}
*/
var batch_values = null;
/** @type {Effect | null} */
var last_scheduled_effect = null;
var is_flushing_sync = false;
var is_processing = false;
/**
* During traversal, this is an array. Newly created effects are (if not immediately
* executed) pushed to this array, rather than going through the scheduling
* rigamarole that would cause another turn of the flush loop.
* @type {Effect[] | null}
*/
var collected_effects = null;
/**
* An array of effects that are marked during traversal as a result of a `set`
* (not `internal_set`) call. These will be added to the next batch and
* trigger another `batch.process()`
* @type {Effect[] | null}
* @deprecated when we get rid of legacy mode and stores, we can get rid of this
*/
var legacy_updates = null;
var flush_count = 0;
var uid = 1;
var Batch = class Batch {
	id = uid++;
	/** True as soon as `#process` was called */
	#started = false;
	linked = true;
	/** @type {Batch | null} */
	#prev = null;
	/** @type {Batch | null} */
	#next = null;
	/** @type {Map<Effect, ReturnType<typeof deferred<any>>>} */
	async_deriveds = /* @__PURE__ */ new Map();
	/**
	* The current values of any signals that are updated in this batch.
	* Tuple format: [value, is_derived] (note: is_derived is false for deriveds, too, if they were overridden via assignment)
	* They keys of this map are identical to `this.#previous`
	* @type {Map<Value, [any, boolean]>}
	*/
	current = /* @__PURE__ */ new Map();
	/**
	* The values of any signals (sources and deriveds) that are updated in this batch _before_ those updates took place.
	* They keys of this map are identical to `this.#current`
	* @type {Map<Value, any>}
	*/
	previous = /* @__PURE__ */ new Map();
	/**
	* When the batch is committed (and the DOM is updated), we need to remove old branches
	* and append new ones by calling the functions added inside (if/each/key/etc) blocks
	* @type {Set<(batch: Batch) => void>}
	*/
	#commit_callbacks = /* @__PURE__ */ new Set();
	/**
	* If a fork is discarded, we need to destroy any effects that are no longer needed
	* @type {Set<(batch: Batch) => void>}
	*/
	#discard_callbacks = /* @__PURE__ */ new Set();
	/**
	* The number of async effects that are currently in flight
	*/
	#pending = 0;
	/**
	* Async effects that are currently in flight, _not_ inside a pending boundary
	* @type {Map<Effect, number>}
	*/
	#blocking_pending = /* @__PURE__ */ new Map();
	/**
	* A deferred that resolves when the batch is committed, used with `settled()`
	* TODO replace with Promise.withResolvers once supported widely enough
	* @type {{ promise: Promise<void>, resolve: (value?: any) => void, reject: (reason: unknown) => void } | null}
	*/
	#deferred = null;
	/**
	* Effects that were scheduled in this batch but not yet 'resolved' into the
	* root effects that need to be flushed. Resolving — the upwards traversal that
	* marks the path to each effect on the shared effect tree (see #resolve) — is
	* deferred until the batch is processed, so that the markers are created and
	* consumed within a single traversal. Scheduling into other batches (which can
	* happen concurrently, e.g. while a batch is committed) can therefore never
	* observe (and be confused by) this batch's markers.
	* May contain duplicates — deduplication happens during resolving
	* @type {Effect[]}
	*/
	#scheduled = [];
	/**
	* Effects created while this batch was active.
	* @type {Effect[]}
	*/
	#new_effects = [];
	/**
	* Deferred effects (which run after async work has completed) that are DIRTY
	* @type {Set<Effect>}
	*/
	#dirty_effects = /* @__PURE__ */ new Set();
	/**
	* Deferred effects that are MAYBE_DIRTY
	* @type {Set<Effect>}
	*/
	#maybe_dirty_effects = /* @__PURE__ */ new Set();
	/**
	* A map of branches that still exist, but will be destroyed when this batch
	* is committed — we skip over these during `process`.
	* The value contains child effects that were dirty/maybe_dirty before being reset,
	* so they can be rescheduled if the branch survives.
	* @type {Map<Effect, { d: Effect[], m: Effect[] }>}
	*/
	#skipped_branches = /* @__PURE__ */ new Map();
	/**
	* Inverse of #skipped_branches which we need to tell prior batches to unskip them when committing
	* @type {Set<Effect>}
	*/
	#unskipped_branches = /* @__PURE__ */ new Set();
	is_fork = false;
	#decrement_queued = false;
	constructor() {
		if (last_batch === null) first_batch = last_batch = this;
		else {
			last_batch.#next = this;
			this.#prev = last_batch;
		}
		last_batch = this;
	}
	#is_deferred() {
		if (this.is_fork) return true;
		for (const effect of this.#blocking_pending.keys()) {
			var e = effect;
			var skipped = false;
			while (e.parent !== null) {
				if (this.#skipped_branches.has(e)) {
					skipped = true;
					break;
				}
				e = e.parent;
			}
			if (!skipped) return true;
		}
		return false;
	}
	/**
	* Add an effect to the #skipped_branches map and reset its children
	* @param {Effect} effect
	*/
	skip_effect(effect) {
		if (!this.#skipped_branches.has(effect)) this.#skipped_branches.set(effect, {
			d: [],
			m: []
		});
		this.#unskipped_branches.delete(effect);
	}
	/**
	* Remove an effect from the #skipped_branches map and reschedule
	* any tracked dirty/maybe_dirty child effects
	* @param {Effect} effect
	* @param {(e: Effect) => void} callback
	*/
	unskip_effect(effect, callback = (e) => this.schedule(e)) {
		var tracked = this.#skipped_branches.get(effect);
		if (tracked) {
			this.#skipped_branches.delete(effect);
			for (var e of tracked.d) {
				set_signal_status(e, DIRTY);
				callback(e);
			}
			for (e of tracked.m) {
				set_signal_status(e, MAYBE_DIRTY);
				callback(e);
			}
		}
		this.#unskipped_branches.add(effect);
	}
	/**
	* Convert the effects that were scheduled in this batch into the root effects
	* that need to be traversed, marking the path to each effect (by clearing the
	* `CLEAN` flag on ancestor branches) so that the traversal can find them.
	* This happens right before traversal rather than at scheduling time, so that
	* the markers left on the (shared) effect tree are created and consumed within
	* a single traversal — scheduling into other batches can never observe them
	* @returns {Effect[]}
	*/
	#resolve() {
		/** @type {Effect[]} */
		var roots = [];
		for (const effect of this.#scheduled) {
			if ((effect.f & 16384) !== 0 || (effect.f & 6144) === 0) continue;
			var e = effect;
			var covered = false;
			while (e.parent !== null) {
				e = e.parent;
				var flags = e.f;
				if ((flags & 96) !== 0) {
					if ((flags & 1024) === 0) {
						covered = true;
						break;
					}
					e.f ^= CLEAN;
				}
			}
			if (!covered) roots.push(e);
		}
		this.#scheduled = [];
		return roots;
	}
	#process() {
		this.#started = true;
		for (const e of this.#dirty_effects) {
			this.#maybe_dirty_effects.delete(e);
			set_signal_status(e, DIRTY);
			this.schedule(e);
		}
		for (const e of this.#maybe_dirty_effects) {
			set_signal_status(e, MAYBE_DIRTY);
			this.schedule(e);
		}
		this.apply();
		/** @type {Effect[]} */
		var effects = collected_effects = [];
		/** @type {Effect[]} */
		var render_effects = [];
		/**
		* @type {Effect[]}
		* @deprecated when we get rid of legacy mode and stores, we can get rid of this
		*/
		var updates = legacy_updates = [];
		while (this.#scheduled.length > 0) {
			if (flush_count++ > 1e3) {
				this.#unlink();
				infinite_loop_guard();
			}
			for (const root of this.#resolve()) try {
				this.#traverse(root, effects, render_effects);
			} catch (e) {
				reset_all(root);
				if (!this.#is_deferred()) this.discard();
				throw e;
			}
		}
		current_batch = null;
		if (updates.length > 0) {
			var batch = Batch.ensure();
			for (const e of updates) batch.schedule(e);
		}
		collected_effects = null;
		legacy_updates = null;
		if (this.#is_deferred()) {
			this.#defer_effects(render_effects);
			this.#defer_effects(effects);
			for (const [e, t] of this.#skipped_branches) reset_branch(e, t);
			if (updates.length > 0)
 /** @type {Batch} */ current_batch.#process();
			return;
		}
		const earlier_batch = this.#find_earlier_batch();
		if (earlier_batch) {
			this.#defer_effects(render_effects);
			this.#defer_effects(effects);
			earlier_batch.#merge(this);
			return;
		}
		this.#dirty_effects.clear();
		this.#maybe_dirty_effects.clear();
		for (const fn of this.#commit_callbacks) fn(this);
		this.#commit_callbacks.clear();
		previous_batch = this;
		flush_queued_effects(render_effects);
		flush_queued_effects(effects);
		previous_batch = null;
		this.#deferred?.resolve();
		var next_batch = current_batch;
		if (this.#pending === 0 && (this.#scheduled.length === 0 || next_batch !== null)) {
			this.#unlink();
			if (async_mode_flag) {
				this.#commit();
				current_batch = next_batch;
			}
		}
		if (this.#scheduled.length > 0) {
			if (next_batch !== null) {
				for (const e of this.#scheduled) next_batch.#scheduled.push(e);
				this.#scheduled = [];
			} else next_batch = this;
		}
		if (next_batch !== null) {
			old_values.clear();
			next_batch.#process();
		}
	}
	/**
	* Traverse the effect tree, executing effects or stashing
	* them for later execution as appropriate
	* @param {Effect} root
	* @param {Effect[]} effects
	* @param {Effect[]} render_effects
	*/
	#traverse(root, effects, render_effects) {
		root.f ^= CLEAN;
		var effect = root.first;
		while (effect !== null) {
			var flags = effect.f;
			var is_branch = (flags & 96) !== 0;
			if (!(is_branch && (flags & 1024) !== 0 || (flags & 8192) !== 0 || this.#skipped_branches.has(effect)) && effect.fn !== null) {
				if (is_branch) effect.f ^= CLEAN;
				else if ((flags & 4) !== 0) effects.push(effect);
				else if (async_mode_flag && (flags & 16777224) !== 0) render_effects.push(effect);
				else if (is_dirty(effect)) {
					if ((flags & 16) !== 0) this.#maybe_dirty_effects.add(effect);
					update_effect(effect);
				}
				var child = effect.first;
				if (child !== null) {
					effect = child;
					continue;
				}
			}
			while (effect !== null) {
				var next = effect.next;
				if (next !== null) {
					effect = next;
					break;
				}
				effect = effect.parent;
			}
		}
	}
	#find_earlier_batch() {
		var batch = this.#prev;
		while (batch !== null) {
			if (!batch.is_fork) {
				for (const [value, [, is_derived]] of this.current) if (batch.current.has(value) && !is_derived) return batch;
			}
			batch = batch.#prev;
		}
		return null;
	}
	/**
	* @param {Batch} batch
	*/
	#merge(batch) {
		for (const [source, value] of batch.current) {
			if (!this.previous.has(source) && batch.previous.has(source)) this.previous.set(source, batch.previous.get(source));
			this.current.set(source, value);
		}
		for (const [effect, deferred] of batch.async_deriveds) {
			const d = this.async_deriveds.get(effect);
			if (d) deferred.promise.then(d.resolve).catch(d.reject);
		}
		batch.async_deriveds.clear();
		this.transfer_effects(batch.#dirty_effects, batch.#maybe_dirty_effects);
		/**
		* mark all effects that depend on `batch.current`, except the
		* async effects that we just resolved (TODO unless they depend
		* on values in this batch that are NOT in the later batch?).
		* Through this we also will populate the correct #skipped_branches,
		* oncommit callbacks etc, so we don't need to merge them separately.
		* @param {Value} value
		*/
		const mark = (value) => {
			var reactions = value.reactions;
			if (reactions === null) return;
			if ((value.f & 2) !== 0 && (value.f & 6144) === 0) return;
			for (const reaction of reactions) {
				var flags = reaction.f;
				if ((flags & 2) !== 0) mark(reaction);
				else {
					var effect = reaction;
					if (flags & 4194320 && !this.async_deriveds.has(effect)) {
						this.#maybe_dirty_effects.delete(effect);
						set_signal_status(effect, DIRTY);
						this.schedule(effect);
					}
				}
			}
		};
		for (const source of this.current.keys()) mark(source);
		this.oncommit(() => batch.discard());
		batch.#unlink();
		current_batch = this;
		this.#process();
	}
	/**
	* @param {Effect[]} effects
	*/
	#defer_effects(effects) {
		for (var i = 0; i < effects.length; i += 1) defer_effect(effects[i], this.#dirty_effects, this.#maybe_dirty_effects);
	}
	/**
	* Associate a change to a given source with the current
	* batch, noting its previous and current values
	* @param {Value} source
	* @param {any} value
	* @param {boolean} [is_derived]
	*/
	capture(source, value, is_derived = false) {
		if (source.v !== UNINITIALIZED && !this.previous.has(source)) this.previous.set(source, source.v);
		if ((source.f & 8388608) === 0) {
			this.current.set(source, [value, is_derived]);
			batch_values?.set(source, value);
		}
		if (!this.is_fork) source.v = value;
	}
	activate() {
		current_batch = this;
	}
	deactivate() {
		current_batch = null;
		batch_values = null;
	}
	flush() {
		try {
			is_processing = true;
			current_batch = this;
			this.#process();
		} finally {
			flush_count = 0;
			last_scheduled_effect = null;
			collected_effects = null;
			legacy_updates = null;
			is_processing = false;
			current_batch = null;
			batch_values = null;
			old_values.clear();
		}
	}
	discard() {
		for (const fn of this.#discard_callbacks) fn(this);
		this.#discard_callbacks.clear();
		for (const deferred of this.async_deriveds.values()) deferred.reject(OBSOLETE);
		this.#unlink();
		this.#deferred?.resolve();
	}
	/**
	* @param {Effect} effect
	*/
	register_created_effect(effect) {
		this.#new_effects.push(effect);
	}
	#commit() {
		for (let batch = first_batch; batch !== null; batch = batch.#next) {
			var is_earlier = batch.id < this.id;
			/** @type {Source[]} */
			var sources = [];
			for (const [source, [value, is_derived]] of this.current) {
				if (batch.current.has(source)) {
					var batch_value = batch.current.get(source)[0];
					if (is_earlier && value !== batch_value) batch.current.set(source, [value, is_derived]);
					else continue;
				}
				sources.push(source);
			}
			if (is_earlier) for (const [effect, deferred] of this.async_deriveds) {
				const d = batch.async_deriveds.get(effect);
				if (d) deferred.promise.then(d.resolve).catch(d.reject);
			}
			var current = [...batch.current.keys()].filter((source) => !batch.current.get(source)[1]);
			if (!batch.#started || current.length === 0) continue;
			var others = current.filter((source) => !this.current.has(source));
			if (others.length === 0) {
				if (is_earlier) batch.discard();
			} else if (sources.length > 0) {
				if (is_earlier) for (const unskipped of this.#unskipped_branches) batch.unskip_effect(unskipped, (e) => {
					if ((e.f & 4194320) !== 0) batch.schedule(e);
					else batch.#defer_effects([e]);
				});
				batch.activate();
				/** @type {Set<Value>} */
				var marked = /* @__PURE__ */ new Set();
				/** @type {Map<Reaction, boolean>} */
				var checked = /* @__PURE__ */ new Map();
				for (var source of sources) mark_effects(source, others, marked, checked);
				checked = /* @__PURE__ */ new Map();
				var current_unequal = [...batch.current].filter(([c, v1]) => {
					const v2 = this.current.get(c);
					if (!v2) return true;
					return v2[0] !== v1[0] || v2[1] !== v1[1];
				}).map(([c]) => c);
				if (current_unequal.length > 0) {
					for (const effect of this.#new_effects) if ((effect.f & 155648) === 0 && depends_on(effect, current_unequal, checked)) {
						if ((effect.f & 4194320) !== 0) {
							set_signal_status(effect, DIRTY);
							batch.schedule(effect);
						} else batch.#dirty_effects.add(effect);
					}
				}
				if (batch.#scheduled.length > 0 && !batch.#decrement_queued) {
					batch.apply();
					for (var root of batch.#resolve()) batch.#traverse(root, [], []);
				}
				batch.deactivate();
			}
		}
	}
	/**
	* @param {boolean} blocking
	* @param {Effect} effect
	*/
	increment(blocking, effect) {
		this.#pending += 1;
		if (blocking) {
			let blocking_pending_count = this.#blocking_pending.get(effect) ?? 0;
			this.#blocking_pending.set(effect, blocking_pending_count + 1);
		}
	}
	/**
	* @param {boolean} blocking
	* @param {Effect} effect
	*/
	decrement(blocking, effect) {
		this.#pending -= 1;
		if (blocking) {
			let blocking_pending_count = this.#blocking_pending.get(effect) ?? 0;
			if (blocking_pending_count === 1) this.#blocking_pending.delete(effect);
			else this.#blocking_pending.set(effect, blocking_pending_count - 1);
		}
		if (this.#decrement_queued) return;
		this.#decrement_queued = true;
		queue_micro_task(() => {
			this.#decrement_queued = false;
			if (this.linked) this.flush();
		});
	}
	/**
	* @param {Set<Effect>} dirty_effects
	* @param {Set<Effect>} maybe_dirty_effects
	*/
	transfer_effects(dirty_effects, maybe_dirty_effects) {
		for (const e of dirty_effects) this.#dirty_effects.add(e);
		for (const e of maybe_dirty_effects) this.#maybe_dirty_effects.add(e);
		dirty_effects.clear();
		maybe_dirty_effects.clear();
	}
	/** @param {(batch: Batch) => void} fn */
	oncommit(fn) {
		this.#commit_callbacks.add(fn);
	}
	/** @param {(batch: Batch) => void} fn */
	ondiscard(fn) {
		this.#discard_callbacks.add(fn);
	}
	settled() {
		return (this.#deferred ??= deferred()).promise;
	}
	static ensure() {
		if (current_batch === null) {
			const batch = current_batch = new Batch();
			if (!is_processing && !is_flushing_sync) queue_micro_task(() => {
				if (!batch.#started) batch.flush();
			});
		}
		return current_batch;
	}
	apply() {
		if (!async_mode_flag || !this.is_fork && this.#prev === null && this.#next === null) {
			batch_values = null;
			return;
		}
		batch_values = /* @__PURE__ */ new Map();
		for (const [source, [value]] of this.current) batch_values.set(source, value);
		for (let batch = first_batch; batch !== null; batch = batch.#next) {
			if (batch === this || batch.is_fork) continue;
			var intersects = false;
			if (batch.id < this.id) for (const [source, [, is_derived]] of batch.current) {
				if (is_derived) continue;
				if (this.current.has(source)) {
					intersects = true;
					break;
				}
			}
			if (!intersects) {
				for (const [source, previous] of batch.previous) if (!batch_values.has(source)) batch_values.set(source, previous);
			}
		}
	}
	/**
	*
	* @param {Effect} effect
	*/
	schedule(effect) {
		last_scheduled_effect = effect;
		if (effect.b?.is_pending && (effect.f & 16777228) !== 0 && (effect.f & 32768) === 0) {
			effect.b.defer_effect(effect);
			return;
		}
		this.#scheduled.push(effect);
	}
	#unlink() {
		if (!this.linked) return;
		var prev = this.#prev;
		var next = this.#next;
		if (prev === null) first_batch = next;
		else prev.#next = next;
		if (next === null) last_batch = prev;
		else next.#prev = prev;
		this.linked = false;
	}
};
/**
* Synchronously flush any pending updates.
* Returns void if no callback is provided, otherwise returns the result of calling the callback.
* @template [T=void]
* @param {(() => T) | undefined} [fn]
* @returns {T}
*/
function flushSync(fn) {
	var was_flushing_sync = is_flushing_sync;
	is_flushing_sync = true;
	try {
		var result;
		if (fn) {
			if (current_batch !== null && !current_batch.is_fork) current_batch.flush();
			result = fn();
		}
		while (true) {
			flush_tasks();
			if (current_batch === null) return result;
			current_batch.flush();
		}
	} finally {
		is_flushing_sync = was_flushing_sync;
	}
}
function infinite_loop_guard() {
	try {
		effect_update_depth_exceeded();
	} catch (error) {
		invoke_error_boundary(error, last_scheduled_effect);
	}
}
/** @type {Set<Effect> | null} */
var eager_block_effects = null;
/**
* @param {Array<Effect>} effects
* @returns {void}
*/
function flush_queued_effects(effects) {
	var length = effects.length;
	if (length === 0) return;
	var i = 0;
	while (i < length) {
		var effect = effects[i++];
		if ((effect.f & 24576) === 0 && is_dirty(effect)) {
			eager_block_effects = /* @__PURE__ */ new Set();
			update_effect(effect);
			if (effect.deps === null && effect.first === null && effect.nodes === null && effect.teardown === null && effect.ac === null) unlink_effect(effect);
			if (eager_block_effects?.size > 0) {
				old_values.clear();
				for (const e of eager_block_effects) {
					if ((e.f & 24576) !== 0) continue;
					/** @type {Effect[]} */
					const ordered_effects = [e];
					let ancestor = e.parent;
					while (ancestor !== null) {
						if (eager_block_effects.has(ancestor)) {
							eager_block_effects.delete(ancestor);
							ordered_effects.push(ancestor);
						}
						ancestor = ancestor.parent;
					}
					for (let j = ordered_effects.length - 1; j >= 0; j--) {
						const e = ordered_effects[j];
						if ((e.f & 24576) !== 0) continue;
						update_effect(e);
					}
				}
				eager_block_effects.clear();
			}
		}
	}
	eager_block_effects = null;
}
/**
* This is similar to `mark_reactions`, but it only marks async/block effects
* depending on `value` and at least one of the other `sources`, so that
* these effects can re-run after another batch has been committed
* @param {Value} value
* @param {Source[]} sources
* @param {Set<Value>} marked
* @param {Map<Reaction, boolean>} checked
*/
function mark_effects(value, sources, marked, checked) {
	if (marked.has(value)) return;
	marked.add(value);
	if (value.reactions !== null) for (const reaction of value.reactions) {
		const flags = reaction.f;
		if ((flags & 2) !== 0) mark_effects(reaction, sources, marked, checked);
		else if ((flags & 4194320) !== 0 && (flags & 2048) === 0 && depends_on(reaction, sources, checked)) {
			set_signal_status(reaction, DIRTY);
			schedule_effect(reaction);
		}
	}
}
/**
* @param {Reaction} reaction
* @param {Source[]} sources
* @param {Map<Reaction, boolean>} checked
*/
function depends_on(reaction, sources, checked) {
	const depends = checked.get(reaction);
	if (depends !== void 0) return depends;
	if (reaction.deps !== null) for (const dep of reaction.deps) {
		if (includes.call(sources, dep)) return true;
		if ((dep.f & 2) !== 0 && depends_on(dep, sources, checked)) {
			checked.set(dep, true);
			return true;
		}
	}
	checked.set(reaction, false);
	return false;
}
/**
* @param {Effect} effect
* @returns {void}
*/
function schedule_effect(effect) {
	/** @type {Batch} */ current_batch.schedule(effect);
}
/**
* Mark all the effects inside a skipped branch CLEAN, so that
* they can be correctly rescheduled later. Tracks dirty and maybe_dirty
* effects so they can be rescheduled if the branch survives.
* @param {Effect} effect
* @param {{ d: Effect[], m: Effect[] }} tracked
*/
function reset_branch(effect, tracked) {
	if ((effect.f & 32) !== 0 && (effect.f & 1024) !== 0) return;
	if ((effect.f & 2048) !== 0) tracked.d.push(effect);
	else if ((effect.f & 4096) !== 0) tracked.m.push(effect);
	set_signal_status(effect, CLEAN);
	var e = effect.first;
	while (e !== null) {
		reset_branch(e, tracked);
		e = e.next;
	}
}
/**
* Mark an entire effect tree clean following an error
* @param {Effect} effect
*/
function reset_all(effect) {
	set_signal_status(effect, CLEAN);
	var e = effect.first;
	while (e !== null) {
		reset_all(e);
		e = e.next;
	}
}
//#endregion
//#region node_modules/svelte/src/internal/client/reactivity/sources.js
/** @import { Derived, Effect, Source, Value } from '#client' */
/** @type {Set<Effect>} */
var eager_effects = /* @__PURE__ */ new Set();
/** @type {Map<Source, any>} */
var old_values = /* @__PURE__ */ new Map();
var eager_effects_deferred = false;
/**
* @template V
* @param {V} v
* @param {Error | null} [stack]
* @returns {Source<V>}
*/
function source(v, stack) {
	return {
		f: 0,
		v,
		reactions: null,
		equals,
		rv: 0,
		wv: 0
	};
}
/**
* @template V
* @param {V} v
* @param {Error | null} [stack]
*/
/*#__NO_SIDE_EFFECTS__*/
function state(v, stack) {
	const s = source(v, stack);
	push_reaction_value(s);
	return s;
}
/**
* @template V
* @param {V} initial_value
* @param {boolean} [immutable]
* @returns {Source<V>}
*/
/*#__NO_SIDE_EFFECTS__*/
function mutable_source(initial_value, immutable = false, trackable = true) {
	const s = source(initial_value);
	if (!immutable) s.equals = safe_equals;
	if (legacy_mode_flag && trackable && component_context !== null && component_context.l !== null) (component_context.l.s ??= []).push(s);
	return s;
}
/**
* @template V
* @param {Source<V>} source
* @param {V} value
* @param {boolean} [should_proxy]
* @returns {V}
*/
function set(source, value, should_proxy = false) {
	if (active_reaction !== null && (!untracking || (active_reaction.f & 131072) !== 0) && is_runes() && (active_reaction.f & 4325394) !== 0 && (current_sources === null || !current_sources.has(source))) state_unsafe_mutation();
	return internal_set(source, should_proxy ? proxy(value) : value, legacy_updates);
}
/**
* A set of signals we have already seen while traversing in mark_reactions.
* Not always set to balance the common case of sources only having a couple
* of (transitive) dependencies (where always creating a Set would be bad for perf)
* with the edge case of extremely deep or wide dependency arrays with cycles.
* @type {Set<any> | null}
*/
var seen = null;
/** Number of transitive dependencies, see {@link seen} for more info */
var count_deps = 0;
/**
* @template V
* @param {Source<V>} source
* @param {V} value
* @param {Effect[] | null} [updated_during_traversal]
* @returns {V}
*/
function internal_set(source, value, updated_during_traversal = null) {
	if (!source.equals(value)) {
		if (is_destroying_effect) old_values.set(source, value);
		else if (!old_values.has(source)) old_values.set(source, source.v);
		var batch = Batch.ensure();
		batch.capture(source, value);
		if ((source.f & 2) !== 0) {
			const derived = source;
			if ((source.f & 2048) !== 0) execute_derived(derived);
			if (batch_values === null) update_derived_status(derived);
		}
		source.wv = increment_write_version();
		seen = null;
		count_deps = 0;
		mark_reactions(source, DIRTY, updated_during_traversal);
		seen = null;
		if (is_runes() && active_effect !== null && (active_effect.f & 1024) !== 0 && (active_effect.f & 96) === 0) {
			if (untracked_writes === null) set_untracked_writes([source]);
			else untracked_writes.push(source);
		}
		if (!batch.is_fork && eager_effects.size > 0 && !eager_effects_deferred) flush_eager_effects();
	}
	return value;
}
function flush_eager_effects() {
	eager_effects_deferred = false;
	for (const effect of eager_effects) {
		if ((effect.f & 1024) !== 0) set_signal_status(effect, MAYBE_DIRTY);
		let dirty;
		try {
			dirty = is_dirty(effect);
		} catch {
			dirty = true;
		}
		if (dirty) update_effect(effect);
	}
	eager_effects.clear();
}
/**
* Silently (without using `get`) increment a source
* @param {Source<number>} source
*/
function increment(source) {
	set(source, source.v + 1);
}
/**
* @param {Value} signal
* @param {number} status should be DIRTY or MAYBE_DIRTY
* @param {Effect[] | null} updated_during_traversal
* @returns {void}
*/
function mark_reactions(signal, status, updated_during_traversal) {
	var reactions = signal.reactions;
	if (reactions === null) return;
	var runes = is_runes();
	var length = reactions.length;
	count_deps += length;
	if (count_deps > 1e5 && seen === null) seen = /* @__PURE__ */ new Set();
	if (seen !== null) {
		if (seen.has(signal)) return;
		seen.add(signal);
	}
	for (var i = 0; i < length; i++) {
		var reaction = reactions[i];
		var flags = reaction.f;
		if (!runes && reaction === active_effect) continue;
		var not_dirty = (flags & DIRTY) === 0;
		if (not_dirty) set_signal_status(reaction, status);
		if ((flags & 131072) !== 0) eager_effects.add(reaction);
		else if ((flags & 2) !== 0) {
			var derived = reaction;
			batch_values?.delete(derived);
			mark_reactions(derived, MAYBE_DIRTY, updated_during_traversal);
		} else if (not_dirty) {
			var effect = reaction;
			if ((flags & 16) !== 0 && eager_block_effects !== null) eager_block_effects.add(effect);
			if (updated_during_traversal !== null) updated_during_traversal.push(effect);
			else schedule_effect(effect);
		}
	}
}
/**
* @template T
* @param {T} value
* @returns {T}
*/
function proxy(value) {
	if (typeof value !== "object" || value === null || STATE_SYMBOL in value || COMPONENT_SYMBOL in value) return value;
	const prototype = get_prototype_of(value);
	if (prototype !== object_prototype && prototype !== array_prototype) return value;
	/** @type {Map<any, Source<any>>} */
	var sources = /* @__PURE__ */ new Map();
	var is_proxied_array = is_array(value);
	var version = /* @__PURE__ */ state(0);
	var stack = null;
	var parent_version = update_version;
	/**
	* Executes the proxy in the context of the reaction it was originally created in, if any
	* @template T
	* @param {() => T} fn
	*/
	var with_parent = (fn) => {
		if (update_version === parent_version) return fn();
		var reaction = active_reaction;
		var version = update_version;
		set_active_reaction(null);
		set_update_version(parent_version);
		var result = fn();
		set_active_reaction(reaction);
		set_update_version(version);
		return result;
	};
	if (is_proxied_array) sources.set("length", /* @__PURE__ */ state(
		/** @type {any[]} */
		value.length,
		stack
	));
	return new Proxy(value, {
		defineProperty(_, prop, descriptor) {
			if (!("value" in descriptor) || descriptor.configurable === false || descriptor.enumerable === false || descriptor.writable === false) state_descriptors_fixed();
			var s = sources.get(prop);
			if (s === void 0) with_parent(() => {
				var s = /* @__PURE__ */ state(descriptor.value, stack);
				sources.set(prop, s);
				return s;
			});
			else set(s, descriptor.value, true);
			return true;
		},
		deleteProperty(target, prop) {
			var s = sources.get(prop);
			if (s === void 0) {
				if (prop in target) {
					const s = with_parent(() => /* @__PURE__ */ state(UNINITIALIZED, stack));
					sources.set(prop, s);
					increment(version);
				}
			} else {
				set(s, UNINITIALIZED);
				increment(version);
			}
			return true;
		},
		get(target, prop, receiver) {
			if (prop === STATE_SYMBOL) return value;
			var s = sources.get(prop);
			var exists = prop in target;
			if (s === void 0 && (!exists || get_descriptor(target, prop)?.writable)) {
				s = with_parent(() => {
					return /* @__PURE__ */ state(proxy(exists ? target[prop] : UNINITIALIZED), stack);
				});
				sources.set(prop, s);
			}
			if (s !== void 0) {
				var v = get(s);
				return v === UNINITIALIZED ? void 0 : v;
			}
			return Reflect.get(target, prop, receiver);
		},
		getOwnPropertyDescriptor(target, prop) {
			this.has?.(target, prop);
			var descriptor = Reflect.getOwnPropertyDescriptor(target, prop);
			var s = sources.get(prop);
			if (s !== void 0) {
				var value = get(s);
				if (value === UNINITIALIZED) return;
				if (descriptor && "value" in descriptor) descriptor.value = value;
				else return {
					enumerable: true,
					configurable: true,
					value,
					writable: true
				};
			}
			return descriptor;
		},
		has(target, prop) {
			if (prop === STATE_SYMBOL) return true;
			var s = sources.get(prop);
			var has = s !== void 0 && s.v !== UNINITIALIZED || Reflect.has(target, prop);
			if (s !== void 0 || active_effect !== null && (!has || get_descriptor(target, prop)?.writable)) {
				if (s === void 0) {
					s = with_parent(() => {
						return /* @__PURE__ */ state(has ? proxy(target[prop]) : UNINITIALIZED, stack);
					});
					sources.set(prop, s);
				}
				if (get(s) === UNINITIALIZED) return false;
			}
			return has;
		},
		set(target, prop, value, receiver) {
			var s = sources.get(prop);
			var has = prop in target;
			if (is_proxied_array && prop === "length") for (var i = value; i < s.v; i += 1) {
				var other_s = sources.get(i + "");
				if (other_s !== void 0) set(other_s, UNINITIALIZED);
				else if (i in target) {
					other_s = with_parent(() => /* @__PURE__ */ state(UNINITIALIZED, stack));
					sources.set(i + "", other_s);
				}
			}
			if (s === void 0) {
				if (!has || get_descriptor(target, prop)?.writable) {
					s = with_parent(() => /* @__PURE__ */ state(void 0, stack));
					set(s, proxy(value));
					sources.set(prop, s);
				}
			} else {
				has = s.v !== UNINITIALIZED;
				var p = with_parent(() => proxy(value));
				set(s, p);
			}
			var descriptor = Reflect.getOwnPropertyDescriptor(target, prop);
			if (descriptor?.set) descriptor.set.call(receiver, value);
			if (!has) {
				if (is_proxied_array && typeof prop === "string") {
					var ls = sources.get("length");
					var n = Number(prop);
					if (Number.isInteger(n) && n >= ls.v) set(ls, n + 1);
				}
				increment(version);
			}
			return true;
		},
		ownKeys(target) {
			get(version);
			var own_keys = Reflect.ownKeys(target).filter((key) => {
				var source = sources.get(key);
				return source === void 0 || source.v !== UNINITIALIZED;
			});
			for (var [key, source] of sources) if (source.v !== UNINITIALIZED && !(key in target)) own_keys.push(key);
			return own_keys;
		},
		setPrototypeOf() {
			state_prototype_fixed();
		}
	});
}
/**
* @param {any} value
*/
function get_proxied_value(value) {
	try {
		if (value !== null && typeof value === "object" && STATE_SYMBOL in value) return value[STATE_SYMBOL];
	} catch {}
	return value;
}
/**
* @param {any} a
* @param {any} b
*/
function is(a, b) {
	return Object.is(get_proxied_value(a), get_proxied_value(b));
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/operations.js
/** @import { Effect, TemplateNode } from '#client' */
/** @type {Window} */
var $window;
/** @type {boolean} */
var is_firefox;
/** @type {() => Node | null} */
var first_child_getter;
/** @type {() => Node | null} */
var next_sibling_getter;
/**
* Initialize these lazily to avoid issues when using the runtime in a server context
* where these globals are not available while avoiding a separate server entry point
*/
function init_operations() {
	if ($window !== void 0) return;
	$window = window;
	is_firefox = /Firefox/.test(navigator.userAgent);
	var element_prototype = Element.prototype;
	var node_prototype = Node.prototype;
	var text_prototype = Text.prototype;
	first_child_getter = get_descriptor(node_prototype, "firstChild").get;
	next_sibling_getter = get_descriptor(node_prototype, "nextSibling").get;
	if (is_extensible(element_prototype)) {
		/** @type {any} */ element_prototype[CLASS_CACHE] = void 0;
		/** @type {any} */ element_prototype[ATTRIBUTES_CACHE] = null;
		/** @type {any} */ element_prototype[STYLE_CACHE] = void 0;
		element_prototype.__e = void 0;
	}
	if (is_extensible(text_prototype))
 /** @type {any} */ text_prototype[TEXT_CACHE] = void 0;
}
/**
* @param {string} value
* @returns {Text}
*/
function create_text(value = "") {
	return document.createTextNode(value);
}
/**
* @template {Node} N
* @param {N} node
*/
/*@__NO_SIDE_EFFECTS__*/
function get_first_child(node) {
	return first_child_getter.call(node);
}
/**
* @template {Node} N
* @param {N} node
*/
/*@__NO_SIDE_EFFECTS__*/
function get_next_sibling(node) {
	return next_sibling_getter.call(node);
}
/**
* Don't mark this as side-effect-free, hydration needs to walk all nodes
* @template {Node} N
* @param {N} node
* @param {boolean} is_text
* @returns {TemplateNode | null}
*/
function child(node, is_text) {
	if (!hydrating) return /* @__PURE__ */ get_first_child(node);
	var child = /* @__PURE__ */ get_first_child(hydrate_node);
	if (child === null) child = hydrate_node.appendChild(create_text());
	else if (is_text && child.nodeType !== 3) {
		var text = create_text();
		child?.before(text);
		set_hydrate_node(text);
		return text;
	}
	if (is_text) merge_text_nodes(child);
	set_hydrate_node(child);
	return child;
}
/**
* Don't mark this as side-effect-free, hydration needs to walk all nodes
* @param {TemplateNode} node
* @param {boolean} [is_text]
* @returns {TemplateNode | null}
*/
function first_child(node, is_text = false) {
	if (!hydrating) {
		var first = /* @__PURE__ */ get_first_child(node);
		if (first instanceof Comment && first.data === "") return /* @__PURE__ */ get_next_sibling(first);
		return first;
	}
	if (is_text) {
		if (hydrate_node?.nodeType !== 3) {
			var text = create_text();
			hydrate_node?.before(text);
			set_hydrate_node(text);
			return text;
		}
		merge_text_nodes(hydrate_node);
	}
	return hydrate_node;
}
/**
* `child`, for the very common case of an element with exactly one child. Resetting the
* hydration cursor is part of the same step, so the compiler doesn't have to emit a
* separate `reset` call for every `<p>{text}</p>` in an app.
* Don't mark this as side-effect-free, hydration needs to walk all nodes
* @param {TemplateNode} node
* @param {boolean} [is_text]
* @returns {TemplateNode | null}
*/
function only_child(node, is_text = false) {
	if (!hydrating) return /* @__PURE__ */ get_first_child(node);
	var first = child(node, is_text);
	reset(node);
	return first;
}
/**
* Don't mark this as side-effect-free, hydration needs to walk all nodes
* @param {TemplateNode} node
* @param {number} count
* @param {boolean} is_text
* @returns {TemplateNode | null}
*/
function sibling(node, count = 1, is_text = false) {
	let next_sibling = hydrating ? hydrate_node : node;
	var last_sibling;
	while (count--) {
		last_sibling = next_sibling;
		next_sibling = /* @__PURE__ */ get_next_sibling(next_sibling);
	}
	if (!hydrating) return next_sibling;
	if (is_text) {
		if (next_sibling?.nodeType !== 3) {
			var text = create_text();
			if (next_sibling === null) last_sibling?.after(text);
			else next_sibling.before(text);
			set_hydrate_node(text);
			return text;
		}
		merge_text_nodes(next_sibling);
	}
	set_hydrate_node(next_sibling);
	return next_sibling;
}
/**
* @template {Node} N
* @param {N} node
* @returns {void}
*/
function clear_text_content(node) {
	node.textContent = "";
}
/**
* Returns `true` if we're updating the current block, for example `condition` in
* an `{#if condition}` block just changed. In this case, the branch should be
* appended (or removed) at the same time as other updates within the
* current `<svelte:boundary>`
*/
function should_defer_append() {
	if (!async_mode_flag) return false;
	if (eager_block_effects !== null) return false;
	return (active_effect.f & REACTION_RAN) !== 0;
}
/**
* Branching here is intentional and load-bearing for perf. `createElement(tag)`
* hits a fast path in Blink that `createElementNS(NAMESPACE_HTML, tag)` doesn't,
* and passing an explicit `undefined` as the trailing options arg measurably
* slows both APIs. Funnelling every case through a single `createElementNS(ns,
* tag, options)` call would be smaller but slower on the HTML path.
*
* @template {keyof HTMLElementTagNameMap | string} T
* @param {T} tag
* @param {string} [namespace]
* @param {string} [is]
* @returns {T extends keyof HTMLElementTagNameMap ? HTMLElementTagNameMap[T] : Element}
*/
function create_element(tag, namespace, is) {
	if (namespace == null || namespace === "http://www.w3.org/1999/xhtml") return is ? document.createElement(tag, { is }) : document.createElement(tag);
	return is ? document.createElementNS(namespace, tag, { is }) : document.createElementNS(namespace, tag);
}
/**
* Browsers split text nodes larger than 65536 bytes when parsing.
* For hydration to succeed, we need to stitch them back together
* @param {Text} text
*/
function merge_text_nodes(text) {
	if (text.nodeValue.length < 65536) return;
	let next = text.nextSibling;
	while (next !== null && next.nodeType === 3) {
		next.remove();
		/** @type {string} */ text.nodeValue += next.nodeValue;
		next = text.nextSibling;
	}
}
/**
* @param {unknown} error
*/
function handle_error(error) {
	var effect = active_effect;
	if (effect === null) {
		/** @type {Derived} */ active_reaction.f |= ERROR_VALUE;
		return error;
	}
	if ((effect.f & 32768) === 0 && (effect.f & 4) === 0) throw error;
	invoke_error_boundary(error, effect);
}
/**
* @param {unknown} error
* @param {Effect | null} effect
*/
function invoke_error_boundary(error, effect) {
	if (effect !== null && (effect.f & 16384) !== 0) return;
	while (effect !== null) {
		if ((effect.f & 128) !== 0 && (effect.f & 33570816) === 0) {
			if ((effect.f & 32768) === 0) throw error;
			try {
				/** @type {Boundary} */ effect.b.error(error);
				return;
			} catch (e) {
				error = e;
			}
		}
		effect = effect.parent;
	}
	throw error;
}
//#endregion
//#region node_modules/svelte/src/internal/client/reactivity/effects.js
/** @import { Blocker, ComponentContext, ComponentContextLegacy, Derived, Effect, TemplateNode, TransitionManager } from '#client' */
/**
* @param {'$effect' | '$effect.pre' | '$inspect'} rune
*/
function validate_effect(rune) {
	if (active_effect === null) {
		if (active_reaction === null) effect_orphan(rune);
		effect_in_unowned_derived();
	}
	if (is_destroying_effect) effect_in_teardown(rune);
}
/**
* @param {Effect} effect
* @param {Effect} parent_effect
*/
function push_effect(effect, parent_effect) {
	var parent_last = parent_effect.last;
	if (parent_last === null) parent_effect.last = parent_effect.first = effect;
	else {
		parent_last.next = effect;
		effect.prev = parent_last;
		parent_effect.last = effect;
	}
}
/**
* @param {number} type
* @param {null | (() => void | (() => void))} fn
* @returns {Effect}
*/
function create_effect(type, fn) {
	var parent = active_effect;
	if (parent !== null && (parent.f & 8192) !== 0) type |= INERT;
	/** @type {Effect} */
	var effect = {
		ctx: component_context,
		deps: null,
		nodes: null,
		f: type | DIRTY | 512,
		first: null,
		fn,
		last: null,
		next: null,
		parent,
		b: parent && parent.b,
		prev: null,
		teardown: null,
		wv: 0,
		ac: null
	};
	current_batch?.register_created_effect(effect);
	/** @type {Effect | null} */
	var e = effect;
	if ((type & 4) !== 0) {
		if (collected_effects !== null) collected_effects.push(effect);
		else Batch.ensure().schedule(effect);
	} else if (fn !== null) {
		try {
			update_effect(effect);
		} catch (e) {
			destroy_effect(effect);
			throw e;
		}
		if (e.deps === null && e.teardown === null && e.nodes === null && e.first === e.last && (e.f & 524288) === 0) {
			e = e.first;
			if ((type & 16) !== 0 && (type & 65536) !== 0 && e !== null) e.f |= EFFECT_TRANSPARENT;
		}
	}
	if (e !== null) {
		e.parent = parent;
		if (parent !== null) push_effect(e, parent);
		if (active_reaction !== null && (active_reaction.f & 2) !== 0 && (type & 64) === 0) {
			var derived = active_reaction;
			(derived.effects ??= []).push(e);
		}
	}
	return effect;
}
/**
* Internal representation of `$effect.tracking()`
* @returns {boolean}
*/
function effect_tracking() {
	return active_reaction !== null && !untracking;
}
/**
* @param {() => void} fn
*/
function teardown(fn) {
	const effect = create_effect(8, null);
	set_signal_status(effect, CLEAN);
	effect.teardown = fn;
	return effect;
}
/**
* Internal representation of `$effect(...)`
* @param {() => void | (() => void)} fn
*/
function user_effect(fn) {
	validate_effect("$effect");
	var flags = active_effect.f;
	if (!active_reaction && (flags & 32) !== 0 && component_context !== null && !component_context.i) {
		var context = component_context;
		(context.e ??= []).push(fn);
	} else return create_user_effect(fn);
}
/**
* @param {() => void | (() => void)} fn
*/
function create_user_effect(fn) {
	return create_effect(4 | USER_EFFECT, fn);
}
/**
* Internal representation of `$effect.pre(...)`
* @param {() => void | (() => void)} fn
* @returns {Effect}
*/
function user_pre_effect(fn) {
	validate_effect("$effect.pre");
	return create_effect(8 | USER_EFFECT, fn);
}
/**
* An effect root whose children can transition out
* @param {() => void} fn
* @returns {(options?: { outro?: boolean }) => Promise<void>}
*/
function component_root(fn) {
	Batch.ensure();
	const effect = create_effect(64 | EFFECT_PRESERVED, fn);
	return (options = {}) => {
		return new Promise((fulfil) => {
			if (options.outro) pause_effect(effect, () => {
				destroy_effect(effect);
				fulfil(void 0);
			});
			else {
				destroy_effect(effect);
				fulfil(void 0);
			}
		});
	};
}
/**
* @param {() => void | (() => void)} fn
* @returns {Effect}
*/
function effect(fn) {
	return create_effect(4, fn);
}
/**
* @param {() => void | (() => void)} fn
* @returns {Effect}
*/
function async_effect(fn) {
	return create_effect(ASYNC | EFFECT_PRESERVED, fn);
}
/**
* @param {() => void | (() => void)} fn
* @returns {Effect}
*/
function render_effect(fn, flags = 0) {
	return create_effect(8 | flags, fn);
}
/**
* @param {(...expressions: any) => void | (() => void)} fn
* @param {Array<() => any>} sync
* @param {Array<() => Promise<any>>} async
* @param {Blocker[]} blockers
*/
function template_effect(fn, sync = [], async = [], blockers = []) {
	flatten(blockers, sync, async, (values) => {
		create_effect(8, () => {
			fn(...values.map(get));
		});
	});
}
/**
* @param {(() => void)} fn
* @param {number} flags
*/
function block(fn, flags = 0) {
	return create_effect(16 | flags, fn);
}
/**
* @param {(() => void)} fn
*/
function branch(fn) {
	return create_effect(32 | EFFECT_PRESERVED, fn);
}
/**
* @param {Effect} effect
*/
function execute_effect_teardown(effect) {
	var teardown = effect.teardown;
	if (teardown !== null) {
		const previously_destroying_effect = is_destroying_effect;
		const previous_reaction = active_reaction;
		set_is_destroying_effect(true);
		set_active_reaction(null);
		try {
			teardown.call(null);
		} catch (error) {
			invoke_error_boundary(error, effect.parent);
		} finally {
			set_is_destroying_effect(previously_destroying_effect);
			set_active_reaction(previous_reaction);
		}
	}
}
/**
* @param {Effect} signal
* @param {boolean} remove_dom
* @returns {void}
*/
function destroy_effect_children(signal, remove_dom = false) {
	var effect = signal.first;
	signal.first = signal.last = null;
	while (effect !== null) {
		const controller = effect.ac;
		if (controller !== null) without_reactive_context(() => {
			controller.abort(STALE_REACTION);
		});
		var next = effect.next;
		if ((effect.f & 64) !== 0) effect.parent = null;
		else destroy_effect(effect, remove_dom);
		effect = next;
	}
}
/**
* @param {Effect} signal
* @returns {void}
*/
function destroy_block_effect_children(signal) {
	var effect = signal.first;
	while (effect !== null) {
		var next = effect.next;
		if ((effect.f & 32) === 0) destroy_effect(effect);
		effect = next;
	}
}
/**
* @param {Effect} effect
* @param {boolean} [remove_dom]
* @returns {void}
*/
function destroy_effect(effect, remove_dom = true) {
	var removed = false;
	if ((remove_dom || (effect.f & 262144) !== 0) && effect.nodes !== null && effect.nodes.end !== null) {
		remove_effect_dom(effect.nodes.start, effect.nodes.end);
		removed = true;
	}
	effect.f |= DESTROYING;
	destroy_effect_children(effect, remove_dom && !removed);
	remove_reactions(effect, 0);
	var transitions = effect.nodes && effect.nodes.t;
	if (transitions !== null) for (const transition of transitions) transition.stop();
	execute_effect_teardown(effect);
	effect.f ^= DESTROYING;
	effect.f |= DESTROYED;
	var parent = effect.parent;
	if (parent !== null && parent.first !== null) unlink_effect(effect);
	effect.next = effect.prev = effect.teardown = effect.ctx = effect.deps = effect.fn = effect.nodes = effect.ac = effect.b = null;
}
/**
*
* @param {TemplateNode | null} node
* @param {TemplateNode} end
*/
function remove_effect_dom(node, end) {
	while (node !== null) {
		/** @type {TemplateNode | null} */
		var next = node === end ? null : /* @__PURE__ */ get_next_sibling(node);
		node.remove();
		node = next;
	}
}
/**
* Detach an effect from the effect tree, freeing up memory and
* reducing the amount of work that happens on subsequent traversals
* @param {Effect} effect
*/
function unlink_effect(effect) {
	var parent = effect.parent;
	var prev = effect.prev;
	var next = effect.next;
	if (prev !== null) prev.next = next;
	if (next !== null) next.prev = prev;
	if (parent !== null) {
		if (parent.first === effect) parent.first = next;
		if (parent.last === effect) parent.last = prev;
	}
}
/**
* When a block effect is removed, we don't immediately destroy it or yank it
* out of the DOM, because it might have transitions. Instead, we 'pause' it.
* It stays around (in memory, and in the DOM) until outro transitions have
* completed, and if the state change is reversed then we _resume_ it.
* A paused effect does not update, and the DOM subtree becomes inert.
* @param {Effect} effect
* @param {() => void} [callback]
* @param {boolean} [destroy]
*/
function pause_effect(effect, callback, destroy = true) {
	/** @type {TransitionManager[]} */
	var transitions = [];
	effect.f |= 256;
	pause_children(effect, transitions, true);
	var fn = () => {
		if (destroy) destroy_effect(effect);
		if (callback) callback();
	};
	var remaining = transitions.length;
	if (remaining > 0) {
		var check = () => --remaining || fn();
		for (var transition of transitions) transition.out(check);
	} else fn();
}
/**
* @param {Effect} effect
* @param {TransitionManager[]} transitions
* @param {boolean} local
*/
function pause_children(effect, transitions, local) {
	if ((effect.f & 8192) !== 0) return;
	effect.f ^= INERT;
	var t = effect.nodes && effect.nodes.t;
	if (t !== null) {
		for (const transition of t) if (transition.is_global || local) transitions.push(transition);
	}
	var child = effect.first;
	while (child !== null) {
		var sibling = child.next;
		if ((child.f & 64) === 0) {
			var transparent = (child.f & 65536) !== 0 || (child.f & 32) !== 0 && (effect.f & 16) !== 0;
			pause_children(child, transitions, transparent ? local : false);
		}
		child = sibling;
	}
}
/**
* The opposite of `pause_effect`. We call this if (for example)
* `x` becomes falsy then truthy: `{#if x}...{/if}`
* @param {Effect} effect
*/
function resume_effect(effect) {
	effect.f &= -257;
	resume_children(effect, true);
}
/**
* @param {Effect} effect
* @param {boolean} local
*/
function resume_children(effect, local) {
	if ((effect.f & 256) !== 0) return;
	if ((effect.f & 8192) === 0) return;
	effect.f ^= INERT;
	if ((effect.f & 1024) === 0) {
		set_signal_status(effect, DIRTY);
		Batch.ensure().schedule(effect);
	}
	var child = effect.first;
	while (child !== null) {
		var sibling = child.next;
		var transparent = (child.f & 65536) !== 0 || (child.f & 32) !== 0;
		resume_children(child, transparent ? local : false);
		child = sibling;
	}
	var t = effect.nodes && effect.nodes.t;
	if (t !== null) {
		for (const transition of t) if (transition.is_global || local) transition.in();
	}
}
/**
* @param {Effect} effect
* @param {DocumentFragment} fragment
*/
function move_effect(effect, fragment) {
	if (!effect.nodes) return;
	/** @type {TemplateNode | null} */
	var node = effect.nodes.start;
	var end = effect.nodes.end;
	while (node !== null) {
		/** @type {TemplateNode | null} */
		var next = node === end ? null : /* @__PURE__ */ get_next_sibling(node);
		fragment.append(node);
		node = next;
	}
}
//#endregion
//#region node_modules/svelte/src/internal/client/legacy.js
/**
* @type {Set<Value> | null}
* @deprecated
*/
var captured_signals = null;
//#endregion
//#region node_modules/svelte/src/internal/client/runtime.js
/** @import { Derived, Effect, Reaction, Source, Value } from '#client' */
/**
* True if updating in an effect context that is reactive (i.e. not branch/root effects)
*/
var is_updating_effect = false;
var is_destroying_effect = false;
/** @param {boolean} value */
function set_is_destroying_effect(value) {
	is_destroying_effect = value;
}
/** @type {null | Reaction} */
var active_reaction = null;
var untracking = false;
/** @param {null | Reaction} reaction */
function set_active_reaction(reaction) {
	active_reaction = reaction;
}
/** @type {null | Effect} */
var active_effect = null;
/** @param {null | Effect} effect */
function set_active_effect(effect) {
	active_effect = effect;
}
/**
* When sources are created within a reaction, reading and writing
* them within that reaction should not cause a re-run
* @type {null | Set<Source>}
*/
var current_sources = null;
/** @param {Value} value */
function push_reaction_value(value) {
	if (active_reaction !== null && (!async_mode_flag && (active_reaction.f & 2097152) !== 0 || (active_reaction.f & 2) !== 0)) (current_sources ??= /* @__PURE__ */ new Set()).add(value);
}
/**
* The dependencies of the reaction that is currently being executed. In many cases,
* the dependencies are unchanged between runs, and so this will be `null` unless
* and until a new dependency is accessed — we track this via `skipped_deps`
* @type {null | Value[]}
*/
var new_deps = null;
var skipped_deps = 0;
/**
* Tracks writes that the effect it's executed in doesn't listen to yet,
* so that the dependency can be added to the effect later on if it then reads it
* @type {null | Source[]}
*/
var untracked_writes = null;
/** @param {null | Source[]} value */
function set_untracked_writes(value) {
	untracked_writes = value;
}
/**
* @type {number} Used by sources and deriveds for handling updates.
* Version starts from 1 so that unowned deriveds differentiate between a created effect and a run one for tracing
**/
var write_version = 1;
/** @type {number} Used to version each read of a source of derived to avoid duplicating dependencies inside a reaction */
var read_version = 0;
var update_version = read_version;
/** @param {number} value */
function set_update_version(value) {
	update_version = value;
}
function increment_write_version() {
	return ++write_version;
}
/**
* Determines whether a derived or effect is dirty.
* If it is MAYBE_DIRTY, will set the status to CLEAN
* @param {Reaction} reaction
* @returns {boolean}
*/
function is_dirty(reaction) {
	var flags = reaction.f;
	if ((flags & 2048) !== 0) return true;
	if ((flags & 4096) !== 0) {
		var dependencies = reaction.deps;
		var length = dependencies.length;
		for (var i = 0; i < length; i++) {
			var dependency = dependencies[i];
			if (is_dirty(dependency)) update_derived(dependency);
			if (dependency.wv > reaction.wv) return true;
		}
		if ((flags & 512) !== 0 && batch_values === null) set_signal_status(reaction, CLEAN);
	}
	return false;
}
/**
* @param {Value} signal
* @param {Effect} effect
* @param {boolean} [root]
*/
function schedule_possible_effect_self_invalidation(signal, effect, root = true) {
	var reactions = signal.reactions;
	if (reactions === null) return;
	if (!async_mode_flag && current_sources !== null && current_sources.has(signal)) return;
	for (var i = 0; i < reactions.length; i++) {
		var reaction = reactions[i];
		if ((reaction.f & 2) !== 0) schedule_possible_effect_self_invalidation(reaction, effect, false);
		else if (effect === reaction) {
			if (root) set_signal_status(reaction, DIRTY);
			else if ((reaction.f & 1024) !== 0) set_signal_status(reaction, MAYBE_DIRTY);
			schedule_effect(reaction);
		}
	}
}
/** @param {Reaction} reaction */
function update_reaction(reaction) {
	var previous_deps = new_deps;
	var previous_skipped_deps = skipped_deps;
	var previous_untracked_writes = untracked_writes;
	var previous_reaction = active_reaction;
	var previous_sources = current_sources;
	var previous_component_context = component_context;
	var previous_untracking = untracking;
	var previous_update_version = update_version;
	var flags = reaction.f;
	new_deps = null;
	skipped_deps = 0;
	untracked_writes = null;
	active_reaction = (flags & 96) === 0 ? reaction : null;
	current_sources = null;
	set_component_context(reaction.ctx);
	untracking = false;
	update_version = ++read_version;
	if (reaction.ac !== null) {
		without_reactive_context(() => {
			/** @type {AbortController} */ reaction.ac.abort(STALE_REACTION);
		});
		reaction.ac = null;
	}
	try {
		reaction.f |= REACTION_IS_UPDATING;
		var fn = reaction.fn;
		var result = fn();
		reaction.f |= REACTION_RAN;
		var deps = update_dependencies(reaction);
		if (is_runes() && untracked_writes !== null && !untracking && deps !== null && (reaction.f & 6146) === 0) for (var i = 0; i < untracked_writes.length; i++) schedule_possible_effect_self_invalidation(untracked_writes[i], reaction);
		if (previous_reaction !== null && previous_reaction !== reaction) {
			read_version++;
			if (previous_reaction.deps !== null) for (let i = 0; i < previous_skipped_deps; i += 1) previous_reaction.deps[i].rv = read_version;
			if (previous_deps !== null) for (const dep of previous_deps) dep.rv = read_version;
			if (untracked_writes !== null) {
				if (previous_untracked_writes === null) previous_untracked_writes = untracked_writes;
				else previous_untracked_writes.push(...untracked_writes);
			}
		}
		if ((reaction.f & 8388608) !== 0) reaction.f ^= ERROR_VALUE;
		return result;
	} catch (error) {
		update_dependencies(reaction);
		return handle_error(error);
	} finally {
		reaction.f ^= REACTION_IS_UPDATING;
		new_deps = previous_deps;
		skipped_deps = previous_skipped_deps;
		untracked_writes = previous_untracked_writes;
		active_reaction = previous_reaction;
		current_sources = previous_sources;
		set_component_context(previous_component_context);
		untracking = previous_untracking;
		update_version = previous_update_version;
	}
}
/**
* @param {Reaction} reaction
*/
function update_dependencies(reaction) {
	var deps = reaction.deps;
	var is_fork = current_batch?.is_fork;
	if (new_deps !== null) {
		var i;
		if (!is_fork) remove_reactions(reaction, skipped_deps);
		if (deps !== null && skipped_deps > 0) {
			deps.length = skipped_deps + new_deps.length;
			for (i = 0; i < new_deps.length; i++) deps[skipped_deps + i] = new_deps[i];
		} else reaction.deps = deps = new_deps;
		if (effect_tracking() && (reaction.f & 512) !== 0) for (i = skipped_deps; i < deps.length; i++) (deps[i].reactions ??= []).push(reaction);
	} else if (!is_fork && deps !== null && skipped_deps < deps.length) {
		remove_reactions(reaction, skipped_deps);
		deps.length = skipped_deps;
	}
	return deps;
}
/**
* @template V
* @param {Reaction} signal
* @param {Value<V>} dependency
* @returns {void}
*/
function remove_reaction(signal, dependency) {
	let reactions = dependency.reactions;
	if (reactions !== null) {
		var index = index_of.call(reactions, signal);
		if (index !== -1) {
			var new_length = reactions.length - 1;
			if (new_length === 0) reactions = dependency.reactions = null;
			else {
				reactions[index] = reactions[new_length];
				reactions.pop();
			}
		}
	}
	if (reactions === null && (dependency.f & 2) !== 0 && (new_deps === null || !includes.call(new_deps, dependency))) {
		var derived = dependency;
		if ((derived.f & 512) !== 0) derived.f ^= 512;
		if (derived.v !== UNINITIALIZED) update_derived_status(derived);
		if (derived.ac !== null) without_reactive_context(() => {
			/** @type {AbortController} */ derived.ac.abort(STALE_REACTION);
			derived.ac = null;
			set_signal_status(derived, DIRTY);
		});
		freeze_derived_effects(derived);
		remove_reactions(derived, 0);
	}
}
/**
* @param {Reaction} signal
* @param {number} start_index
* @returns {void}
*/
function remove_reactions(signal, start_index) {
	var dependencies = signal.deps;
	if (dependencies === null) return;
	for (var i = start_index; i < dependencies.length; i++) remove_reaction(signal, dependencies[i]);
}
/**
* @param {Effect} effect
* @returns {void}
*/
function update_effect(effect) {
	var flags = effect.f;
	if ((flags & 16384) !== 0) return;
	set_signal_status(effect, CLEAN);
	var previous_effect = active_effect;
	var was_updating_effect = is_updating_effect;
	active_effect = effect;
	is_updating_effect = (flags & 96) === 0;
	try {
		if ((flags & 16777232) !== 0) destroy_block_effect_children(effect);
		else destroy_effect_children(effect);
		execute_effect_teardown(effect);
		var teardown = update_reaction(effect);
		effect.teardown = typeof teardown === "function" ? teardown : null;
		effect.wv = write_version;
	} finally {
		is_updating_effect = was_updating_effect;
		active_effect = previous_effect;
	}
}
/**
* Returns a promise that resolves once any pending state changes have been applied.
* @returns {Promise<void>}
*/
async function tick() {
	if (async_mode_flag) return new Promise((f) => {
		requestAnimationFrame(() => f());
		setTimeout(() => f());
	});
	await Promise.resolve();
	flushSync();
}
/**
* @template V
* @param {Value<V>} signal
* @returns {V}
*/
function get(signal) {
	var is_derived = (signal.f & 2) !== 0;
	captured_signals?.add(signal);
	if (active_reaction !== null && !untracking) {
		if (!(active_effect !== null && (active_effect.f & 16384) !== 0) && (current_sources === null || !current_sources.has(signal))) {
			var deps = active_reaction.deps;
			if ((active_reaction.f & 2097152) !== 0) {
				if (signal.rv < read_version) {
					signal.rv = read_version;
					if (new_deps === null && deps !== null && deps[skipped_deps] === signal) skipped_deps++;
					else if (new_deps === null) new_deps = [signal];
					else new_deps.push(signal);
				}
			} else {
				active_reaction.deps ??= [];
				if (!includes.call(active_reaction.deps, signal)) active_reaction.deps.push(signal);
				var reactions = signal.reactions;
				if (reactions === null) signal.reactions = [active_reaction];
				else if (!includes.call(reactions, active_reaction)) reactions.push(active_reaction);
			}
		}
	}
	if (is_destroying_effect && old_values.has(signal)) return old_values.get(signal);
	if (is_derived) {
		var derived = signal;
		if (is_destroying_effect) {
			var value = derived.v;
			if ((derived.f & 1024) === 0 && derived.reactions !== null || depends_on_old_values(derived)) value = execute_derived(derived);
			old_values.set(derived, value);
			return value;
		}
		var should_connect = (derived.f & 512) === 0 && !untracking && active_reaction !== null && (is_updating_effect || (active_reaction.f & 512) !== 0);
		var is_new = (derived.f & REACTION_RAN) === 0;
		if (is_dirty(derived)) {
			if (should_connect) derived.f |= 512;
			update_derived(derived);
		}
		if (should_connect && !is_new) {
			unfreeze_derived_effects(derived);
			reconnect(derived);
		}
	}
	if (batch_values?.has(signal)) return batch_values.get(signal);
	if ((signal.f & 8388608) !== 0) throw signal.v;
	return signal.v;
}
/**
* (Re)connect a disconnected derived, so that it is notified
* of changes in `mark_reactions`
* @param {Derived} derived
*/
function reconnect(derived) {
	derived.f |= 512;
	if (derived.deps === null) return;
	for (const dep of derived.deps) {
		(dep.reactions ??= []).push(derived);
		if ((dep.f & 2) !== 0 && (dep.f & 512) === 0) {
			unfreeze_derived_effects(dep);
			reconnect(dep);
		}
	}
}
/** @param {Derived} derived */
function depends_on_old_values(derived) {
	if (derived.v === UNINITIALIZED) return true;
	if (derived.deps === null) return false;
	for (const dep of derived.deps) {
		if (old_values.has(dep)) return true;
		if ((dep.f & 2) !== 0 && depends_on_old_values(dep)) return true;
	}
	return false;
}
/**
* When used inside a [`$derived`](https://svelte.dev/docs/svelte/$derived) or [`$effect`](https://svelte.dev/docs/svelte/$effect),
* any state read inside `fn` will not be treated as a dependency.
*
* ```ts
* $effect(() => {
*   // this will run when `data` changes, but not when `time` changes
*   save(data, {
*     timestamp: untrack(() => time)
*   });
* });
* ```
* @template T
* @param {() => T} fn
* @returns {T}
*/
function untrack(fn) {
	var previous_untracking = untracking;
	try {
		untracking = true;
		return fn();
	} finally {
		untracking = previous_untracking;
	}
}
/**
* Possibly traverse an object and read all its properties so that they're all reactive in case this is `$state`.
* Does only check first level of an object for performance reasons (heuristic should be good for 99% of all cases).
* @param {any} value
* @returns {void}
*/
function deep_read_state(value) {
	if (typeof value !== "object" || !value || value instanceof EventTarget) return;
	if (STATE_SYMBOL in value) deep_read(value);
	else if (!Array.isArray(value)) for (let key in value) {
		const prop = value[key];
		if (typeof prop === "object" && prop && STATE_SYMBOL in prop) deep_read(prop);
	}
}
/**
* Deeply traverse an object and read all its properties
* so that they're all reactive in case this is `$state`
* @param {any} value
* @param {Set<any>} visited
* @returns {void}
*/
function deep_read(value, visited = /* @__PURE__ */ new Set()) {
	if (typeof value === "object" && value !== null && !(value instanceof EventTarget) && !visited.has(value)) {
		visited.add(value);
		if (value instanceof Date) value.getTime();
		for (let key in value) try {
			deep_read(value[key], visited);
		} catch (e) {}
		const proto = get_prototype_of(value);
		if (proto !== Object.prototype && proto !== Array.prototype && proto !== Map.prototype && proto !== Set.prototype && proto !== Date.prototype) {
			const descriptors = get_descriptors(proto);
			for (let key in descriptors) {
				const get = descriptors[key].get;
				if (get) try {
					get.call(value);
				} catch (e) {}
			}
		}
	}
}
/**
* Subset of delegated events which should be passive by default.
* These two are already passive via browser defaults on window, document and body.
* But since
* - we're delegating them
* - they happen often
* - they apply to mobile which is generally less performant
* we're marking them as passive by default for other elements, too.
*/
var PASSIVE_EVENTS = ["touchstart", "touchmove"];
/**
* Returns `true` if `name` is a passive event
* @param {string} name
*/
function is_passive_event(name) {
	return PASSIVE_EVENTS.includes(name);
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/elements/events.js
/**
* Used on elements, as a map of event type -> event handler,
* and on events themselves to track which element handled an event
*/
var event_symbol = Symbol("events");
/** @type {Set<string>} */
var all_registered_events = /* @__PURE__ */ new Set();
/** @type {Set<(events: Array<string>) => void>} */
var root_event_handles = /* @__PURE__ */ new Set();
/**
* @param {string} event_name
* @param {EventTarget} dom
* @param {EventListener} [handler]
* @param {AddEventListenerOptions} [options]
*/
function create_event(event_name, dom, handler, options = {}) {
	/**
	* @this {EventTarget}
	*/
	function target_handler(event) {
		if (!options.capture) handle_event_propagation.call(dom, event);
		if (!event.cancelBubble) return without_reactive_context(() => {
			return handler?.call(this, event);
		});
	}
	if (event_name.startsWith("pointer") || event_name.startsWith("touch") || event_name === "wheel") {
		target_handler.__removed = false;
		queue_micro_task(() => {
			if (!target_handler.__removed) dom.addEventListener(event_name, target_handler, options);
		});
	} else dom.addEventListener(event_name, target_handler, options);
	return target_handler;
}
/**
* @param {string} event_name
* @param {Element} dom
* @param {EventListener} [handler]
* @param {boolean} [capture]
* @param {boolean} [passive]
* @returns {void}
*/
function event(event_name, dom, handler, capture, passive) {
	var options = {
		capture,
		passive
	};
	var target_handler = create_event(event_name, dom, handler, options);
	if (dom === document.body || dom === window || dom === document || dom instanceof HTMLMediaElement) teardown(() => {
		target_handler.__removed = true;
		dom.removeEventListener(event_name, target_handler, options);
	});
}
/**
* @param {string} event_name
* @param {Element} element
* @param {EventListener} [handler]
* @returns {void}
*/
function delegated(event_name, element, handler) {
	(element[event_symbol] ??= {})[event_name] = handler;
}
/**
* @param {Array<string>} events
* @returns {void}
*/
function delegate(events) {
	for (var i = 0; i < events.length; i++) all_registered_events.add(events[i]);
	for (var fn of root_event_handles) fn(events);
}
var last_propagated_event = null;
var last_propagated_event_clear_scheduled = false;
/**
* @this {EventTarget}
* @param {Event} event
* @returns {void}
*/
function handle_event_propagation(event) {
	var handler_element = this;
	var owner_document = handler_element.ownerDocument;
	var event_name = event.type;
	var path = event.composedPath?.() || [];
	var current_target = path[0] || event.target;
	last_propagated_event = event;
	if (!last_propagated_event_clear_scheduled) {
		last_propagated_event_clear_scheduled = true;
		setTimeout(() => {
			last_propagated_event_clear_scheduled = false;
			last_propagated_event = null;
		});
	}
	var path_idx = 0;
	var handled_at = last_propagated_event === event && event[event_symbol];
	if (handled_at) {
		var at_idx = path.indexOf(handled_at);
		if (at_idx !== -1 && (handler_element === document || handler_element === window)) {
			event[event_symbol] = handler_element;
			return;
		}
		var handler_idx = path.indexOf(handler_element);
		if (handler_idx === -1) return;
		if (at_idx <= handler_idx) path_idx = at_idx;
	}
	current_target = path[path_idx] || event.target;
	if (current_target === handler_element) return;
	define_property(event, "currentTarget", {
		configurable: true,
		get() {
			return current_target || owner_document;
		}
	});
	var previous_reaction = active_reaction;
	var previous_effect = active_effect;
	set_active_reaction(null);
	set_active_effect(null);
	try {
		/**
		* @type {unknown}
		*/
		var throw_error;
		/**
		* @type {unknown[]}
		*/
		var other_errors = [];
		while (current_target !== null) {
			if (current_target === handler_element) break;
			try {
				var delegated = current_target[event_symbol]?.[event_name];
				if (delegated != null && (!current_target.disabled || event.target === current_target)) delegated.call(current_target, event);
			} catch (error) {
				if (throw_error) other_errors.push(error);
				else throw_error = error;
			}
			if (event.cancelBubble) break;
			path_idx++;
			current_target = path_idx < path.length ? path[path_idx] : null;
		}
		if (throw_error) {
			for (let error of other_errors) queueMicrotask(() => {
				throw error;
			});
			throw throw_error;
		}
	} finally {
		event[event_symbol] = handler_element;
		delete event.currentTarget;
		set_active_reaction(previous_reaction);
		set_active_effect(previous_effect);
	}
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/reconciler.js
var policy = globalThis?.window?.trustedTypes && /* @__PURE__ */ globalThis.window.trustedTypes.createPolicy("svelte-trusted-html", { 
/** @param {string} html */
createHTML: (html) => {
	return html;
} });
/** @param {string} html */
function create_trusted_html(html) {
	return policy?.createHTML(html) ?? html;
}
/**
* @param {string} html
*/
function create_fragment_from_html(html) {
	var elem = create_element("template");
	elem.innerHTML = create_trusted_html(html.replaceAll("<!>", "<!---->"));
	return elem.content;
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/template.js
/** @import { Effect, EffectNodes, TemplateNode } from '#client' */
/** @import { TemplateStructure } from './types' */
/**
* @param {TemplateNode} start
* @param {TemplateNode | null} end
*/
function assign_nodes(start, end) {
	var effect = active_effect;
	if (effect.nodes === null) effect.nodes = {
		start,
		end,
		a: null,
		t: null
	};
}
/**
* @param {string} content
* @param {number} flags
* @returns {() => Node | Node[]}
*/
/*#__NO_SIDE_EFFECTS__*/
function from_html(content, flags) {
	var is_fragment = (flags & 1) !== 0;
	var use_import_node = (flags & 2) !== 0;
	/** @type {Node} */
	var node;
	/**
	* Whether or not the first item is a text/element node. If not, we need to
	* create an additional comment node to act as `effect.nodes.start`
	*/
	var has_start = !content.startsWith("<!>");
	return () => {
		if (hydrating) {
			assign_nodes(hydrate_node, null);
			return hydrate_node;
		}
		if (node === void 0) {
			node = create_fragment_from_html(has_start ? content : "<!>" + content);
			if (!is_fragment) node = /* @__PURE__ */ get_first_child(node);
		}
		var clone = use_import_node || is_firefox ? document.importNode(node, true) : node.cloneNode(true);
		if (is_fragment) {
			var start = /* @__PURE__ */ get_first_child(clone);
			var end = clone.lastChild;
			assign_nodes(start, end);
		} else assign_nodes(clone, clone);
		return clone;
	};
}
/**
* @param {string} content
* @param {number} flags
* @param {'svg' | 'math'} ns
* @returns {() => Node | Node[]}
*/
/*#__NO_SIDE_EFFECTS__*/
function from_namespace(content, flags, ns = "svg") {
	/**
	* Whether or not the first item is a text/element node. If not, we need to
	* create an additional comment node to act as `effect.nodes.start`
	*/
	var has_start = !content.startsWith("<!>");
	var is_fragment = (flags & 1) !== 0;
	var wrapped = `<${ns}>${has_start ? content : "<!>" + content}</${ns}>`;
	/** @type {Element | DocumentFragment} */
	var node;
	return () => {
		if (hydrating) {
			assign_nodes(hydrate_node, null);
			return hydrate_node;
		}
		if (!node) {
			var root = /* @__PURE__ */ get_first_child(create_fragment_from_html(wrapped));
			if (is_fragment) {
				node = document.createDocumentFragment();
				while (/* @__PURE__ */ get_first_child(root)) node.appendChild(/* @__PURE__ */ get_first_child(root));
			} else node = /* @__PURE__ */ get_first_child(root);
		}
		var clone = node.cloneNode(true);
		if (is_fragment) {
			var start = /* @__PURE__ */ get_first_child(clone);
			var end = clone.lastChild;
			assign_nodes(start, end);
		} else assign_nodes(clone, clone);
		return clone;
	};
}
/**
* @param {string} content
* @param {number} flags
*/
/*#__NO_SIDE_EFFECTS__*/
function from_svg(content, flags) {
	return /* @__PURE__ */ from_namespace(content, flags, "svg");
}
/**
* Don't mark this as side-effect-free, hydration needs to walk all nodes
* @param {any} value
*/
function text(value = "") {
	if (!hydrating) {
		var t = create_text(value + "");
		assign_nodes(t, t);
		return t;
	}
	var node = hydrate_node;
	if (node.nodeType !== 3) {
		node.before(node = create_text());
		set_hydrate_node(node);
	} else merge_text_nodes(node);
	assign_nodes(node, node);
	return node;
}
/**
* @returns {TemplateNode | DocumentFragment}
*/
function comment() {
	if (hydrating) {
		assign_nodes(hydrate_node, null);
		return hydrate_node;
	}
	var frag = document.createDocumentFragment();
	var start = document.createComment("");
	var anchor = create_text();
	frag.append(start, anchor);
	assign_nodes(start, anchor);
	return frag;
}
/**
* Assign the created (or in hydration mode, traversed) dom elements to the current block
* and insert the elements into the dom (in client mode).
* @param {Text | Comment | Element} anchor
* @param {DocumentFragment | Element} dom
*/
function append(anchor, dom) {
	if (hydrating) {
		var effect = active_effect;
		if ((effect.f & 32768) === 0 || effect.nodes.end === null) effect.nodes.end = hydrate_node;
		hydrate_next();
		return;
	}
	if (anchor === null) return;
	anchor.before(dom);
}
//#endregion
//#region node_modules/svelte/src/reactivity/create-subscriber.js
/**
* Returns a `subscribe` function that integrates external event-based systems with Svelte's reactivity.
* It's particularly useful for integrating with web APIs like `MediaQuery`, `IntersectionObserver`, or `WebSocket`.
*
* If `subscribe` is called inside an effect (including indirectly, for example inside a getter),
* the `start` callback will be called with an `update` function. Whenever `update` is called, the effect re-runs.
*
* If `start` returns a cleanup function, it will be called when the effect is destroyed.
*
* If `subscribe` is called in multiple effects, `start` will only be called once as long as the effects
* are active, and the returned teardown function will only be called when all effects are destroyed.
*
* It's best understood with an example. Here's an implementation of [`MediaQuery`](https://svelte.dev/docs/svelte/svelte-reactivity#MediaQuery):
*
* ```js
* import { createSubscriber } from 'svelte/reactivity';
* import { on } from 'svelte/events';
*
* export class MediaQuery {
* 	#query;
* 	#subscribe;
*
* 	constructor(query) {
* 		this.#query = window.matchMedia(`(${query})`);
*
* 		this.#subscribe = createSubscriber((update) => {
* 			// when the `change` event occurs, re-run any effects that read `this.current`
* 			const off = on(this.#query, 'change', update);
*
* 			// stop listening when all the effects are destroyed
* 			return () => off();
* 		});
* 	}
*
* 	get current() {
* 		// This makes the getter reactive, if read in an effect
* 		this.#subscribe();
*
* 		// Return the current state of the query, whether or not we're in an effect
* 		return this.#query.matches;
* 	}
* }
* ```
* @param {(update: () => void) => (() => void) | void} start
* @since 5.7.0
*/
function createSubscriber(start) {
	let subscribers = 0;
	let version = source(0);
	/** @type {(() => void) | void} */
	let stop;
	return () => {
		if (effect_tracking()) {
			get(version);
			render_effect(() => {
				if (subscribers === 0) stop = untrack(() => start(() => increment(version)));
				subscribers += 1;
				return () => {
					queue_micro_task(() => {
						subscribers -= 1;
						if (subscribers === 0) {
							stop?.();
							stop = void 0;
							increment(version);
						}
					});
				};
			});
		}
	};
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/blocks/boundary.js
/** @import { Effect, Source, TemplateNode, } from '#client' */
/**
* @typedef {{
* 	 onerror?: ((error: unknown, reset: () => void) => void) | null;
*   failed?: ((anchor: Node, error: () => unknown, reset: () => () => void) => void) | null;
*   pending?: ((anchor: Node) => void) | null;
* }} BoundaryProps
*/
var flags = EFFECT_TRANSPARENT | EFFECT_PRESERVED;
/**
* @param {TemplateNode} node
* @param {BoundaryProps} props
* @param {((anchor: Node) => void)} children
* @param {((error: unknown) => unknown) | undefined} [transform_error]
* @returns {void}
*/
function boundary(node, props, children, transform_error) {
	new Boundary(node, props, children, transform_error);
}
var Boundary = class {
	/** @type {Boundary | null} */
	parent;
	is_pending = false;
	/**
	* API-level transformError transform function. Transforms errors before they reach the `failed` snippet.
	* Inherited from parent boundary, or defaults to identity.
	* @type {(error: unknown) => unknown}
	*/
	transform_error;
	/** @type {TemplateNode} */
	#anchor;
	/** @type {TemplateNode | null} */
	#hydrate_open = hydrating ? hydrate_node : null;
	/** @type {BoundaryProps} */
	#props;
	/** @type {((anchor: Node) => void)} */
	#children;
	/** @type {Effect} */
	#effect;
	/** @type {Effect | null} */
	#main_effect = null;
	/** @type {Effect | null} */
	#pending_effect = null;
	/** @type {Effect | null} */
	#failed_effect = null;
	/** @type {DocumentFragment | null} */
	#offscreen_fragment = null;
	#local_pending_count = 0;
	#pending_count = 0;
	#pending_count_update_queued = false;
	/** @type {Set<Effect>} */
	#dirty_effects = /* @__PURE__ */ new Set();
	/** @type {Set<Effect>} */
	#maybe_dirty_effects = /* @__PURE__ */ new Set();
	/**
	* A source containing the number of pending async deriveds/expressions.
	* Only created if `$effect.pending()` is used inside the boundary,
	* otherwise updating the source results in needless `Batch.ensure()`
	* calls followed by no-op flushes
	* @type {Source<number> | null}
	*/
	#effect_pending = null;
	#effect_pending_subscriber = createSubscriber(() => {
		this.#effect_pending = source(this.#local_pending_count);
		return () => {
			this.#effect_pending = null;
		};
	});
	/**
	* @param {TemplateNode} node
	* @param {BoundaryProps} props
	* @param {((anchor: Node) => void)} children
	* @param {((error: unknown) => unknown) | undefined} [transform_error]
	*/
	constructor(node, props, children, transform_error) {
		this.#anchor = node;
		this.#props = props;
		this.#children = (anchor) => {
			var effect = active_effect;
			effect.b = this;
			effect.f |= 128;
			children(anchor);
		};
		this.parent = active_effect.b;
		this.transform_error = transform_error ?? this.parent?.transform_error ?? ((e) => e);
		this.#effect = block(() => {
			if (hydrating) {
				const comment = this.#hydrate_open;
				hydrate_next();
				const server_rendered_pending = comment.data === "[!";
				if (comment.data.startsWith("[?")) {
					const serialized_error = JSON.parse(comment.data.slice(2));
					this.#hydrate_failed_content(serialized_error);
				} else if (server_rendered_pending) this.#hydrate_pending_content();
				else this.#hydrate_resolved_content();
			} else this.#render();
		}, flags);
		if (hydrating) this.#anchor = hydrate_node;
	}
	#hydrate_resolved_content() {
		try {
			this.#main_effect = branch(() => this.#children(this.#anchor));
		} catch (error) {
			this.error(error);
		}
	}
	/**
	* @param {unknown} error The deserialized error from the server's hydration comment
	*/
	#hydrate_failed_content(error) {
		const failed = this.#props.failed;
		const { reset, invoke_onerror } = this.#create_reset(error);
		queue_micro_task(invoke_onerror);
		if (!failed) return;
		this.#failed_effect = branch(() => {
			failed(this.#anchor, () => error, () => reset);
		});
	}
	/**
	* Creates the `reset` function for a failed boundary, along with a function
	* that invokes `onerror` with it (if provided)
	* @param {unknown} error
	* @returns {{ reset: () => void, invoke_onerror: () => void }}
	*/
	#create_reset(error) {
		var did_reset = false;
		var calling_on_error = false;
		const reset = () => {
			if (did_reset) {
				svelte_boundary_reset_noop();
				return;
			}
			did_reset = true;
			if (calling_on_error) svelte_boundary_reset_onerror();
			if (this.#failed_effect !== null) pause_effect(this.#failed_effect, () => {
				this.#failed_effect = null;
			});
			this.#run(() => {
				this.#render();
			});
		};
		const invoke_onerror = () => {
			try {
				calling_on_error = true;
				this.#props.onerror?.(error, reset);
				calling_on_error = false;
			} catch (err) {
				invoke_error_boundary(err, this.#effect && this.#effect.parent);
			}
		};
		return {
			reset,
			invoke_onerror
		};
	}
	#hydrate_pending_content() {
		const pending = this.#props.pending;
		if (!pending) return;
		this.is_pending = true;
		this.#pending_effect = branch(() => pending(this.#anchor));
		queue_micro_task(() => {
			var fragment = this.#offscreen_fragment = document.createDocumentFragment();
			var anchor = create_text();
			var handled = false;
			fragment.append(anchor);
			this.#main_effect = this.#run(() => {
				try {
					return branch(() => this.#children(anchor));
				} catch (error) {
					try {
						this.error(error);
						handled = true;
					} catch (error) {
						invoke_error_boundary(error, this.#effect.parent);
					}
					return null;
				}
			});
			if (this.#main_effect === null) {
				this.#offscreen_fragment = null;
				if (handled) this.#resolve(current_batch);
				return;
			}
			if (this.#pending_count === 0) {
				this.#anchor.before(fragment);
				this.#offscreen_fragment = null;
				pause_effect(this.#pending_effect, () => {
					this.#pending_effect = null;
				});
				this.#resolve(current_batch);
			}
		});
	}
	#render() {
		try {
			this.is_pending = this.has_pending_snippet();
			this.#pending_count = 0;
			this.#local_pending_count = 0;
			this.#main_effect = branch(() => {
				this.#children(this.#anchor);
			});
			if (this.#pending_count > 0) {
				var fragment = this.#offscreen_fragment = document.createDocumentFragment();
				move_effect(this.#main_effect, fragment);
				const pending = this.#props.pending;
				this.#pending_effect = branch(() => pending(this.#anchor));
			} else this.#resolve(current_batch);
		} catch (error) {
			this.error(error);
		}
	}
	/**
	* @param {Batch} batch
	*/
	#resolve(batch) {
		this.is_pending = false;
		batch.transfer_effects(this.#dirty_effects, this.#maybe_dirty_effects);
	}
	/**
	* Defer an effect inside a pending boundary until the boundary resolves
	* @param {Effect} effect
	*/
	defer_effect(effect) {
		defer_effect(effect, this.#dirty_effects, this.#maybe_dirty_effects);
	}
	/**
	* Returns `false` if the effect exists inside a boundary whose pending snippet is shown
	* @returns {boolean}
	*/
	is_rendered() {
		return !this.is_pending && (!this.parent || this.parent.is_rendered());
	}
	has_pending_snippet() {
		return !!this.#props.pending;
	}
	/**
	* @template T
	* @param {() => T} fn
	*/
	#run(fn) {
		var previous_effect = active_effect;
		var previous_reaction = active_reaction;
		var previous_ctx = component_context;
		set_active_effect(this.#effect);
		set_active_reaction(this.#effect);
		set_component_context(this.#effect.ctx);
		try {
			Batch.ensure();
			return fn();
		} finally {
			set_active_effect(previous_effect);
			set_active_reaction(previous_reaction);
			set_component_context(previous_ctx);
		}
	}
	/**
	* Updates the pending count associated with the currently visible pending snippet,
	* if any, such that we can replace the snippet with content once work is done
	* @param {1 | -1} d
	* @param {Batch} batch
	*/
	#update_pending_count(d, batch) {
		if (!this.has_pending_snippet()) {
			if (this.parent) this.parent.#update_pending_count(d, batch);
			return;
		}
		this.#pending_count += d;
		if (this.#pending_count === 0) {
			this.#resolve(batch);
			if (this.#pending_effect) pause_effect(this.#pending_effect, () => {
				this.#pending_effect = null;
			});
			if (this.#offscreen_fragment) {
				this.#anchor.before(this.#offscreen_fragment);
				this.#offscreen_fragment = null;
			}
		}
	}
	/**
	* Update the source that powers `$effect.pending()` inside this boundary,
	* and controls when the current `pending` snippet (if any) is removed.
	* Do not call from inside the class
	* @param {1 | -1} d
	* @param {Batch} batch
	*/
	update_pending_count(d, batch) {
		this.#update_pending_count(d, batch);
		this.#local_pending_count += d;
		if (!this.#effect_pending || this.#pending_count_update_queued) return;
		this.#pending_count_update_queued = true;
		queue_micro_task(() => {
			this.#pending_count_update_queued = false;
			if (this.#effect_pending) internal_set(this.#effect_pending, this.#local_pending_count);
		});
	}
	get_effect_pending() {
		this.#effect_pending_subscriber();
		return get(this.#effect_pending);
	}
	/** @param {unknown} error */
	error(error) {
		if (!this.#props.onerror && !this.#props.failed) throw error;
		if (current_batch?.is_fork) {
			if (this.#main_effect) current_batch.skip_effect(this.#main_effect);
			if (this.#pending_effect) current_batch.skip_effect(this.#pending_effect);
			if (this.#failed_effect) current_batch.skip_effect(this.#failed_effect);
			current_batch.oncommit(() => {
				this.#handle_error(error);
			});
		} else this.#handle_error(error);
	}
	/**
	* @param {unknown} error
	*/
	#handle_error(error) {
		if (this.#main_effect) {
			destroy_effect(this.#main_effect);
			this.#main_effect = null;
		}
		if (this.#pending_effect) {
			destroy_effect(this.#pending_effect);
			this.#pending_effect = null;
		}
		if (this.#failed_effect) {
			destroy_effect(this.#failed_effect);
			this.#failed_effect = null;
		}
		if (hydrating) {
			set_hydrate_node(this.#hydrate_open);
			next();
			set_hydrate_node(skip_nodes());
		}
		let failed = this.#props.failed;
		/** @param {unknown} transformed_error */
		const handle_error_result = (transformed_error) => {
			const { reset, invoke_onerror } = this.#create_reset(transformed_error);
			invoke_onerror();
			if (failed) this.#failed_effect = this.#run(() => {
				try {
					return branch(() => {
						var effect = active_effect;
						effect.b = this;
						effect.f |= 128;
						failed(this.#anchor, () => transformed_error, () => reset);
					});
				} catch (error) {
					invoke_error_boundary(error, this.#effect.parent);
					return null;
				}
			});
		};
		queue_micro_task(() => {
			/** @type {unknown} */
			var result;
			try {
				result = this.transform_error(error);
			} catch (e) {
				invoke_error_boundary(e, this.#effect && this.#effect.parent);
				return;
			}
			if (result !== null && typeof result === "object" && typeof result.then === "function")
 /** @type {any} */ result.then(
				handle_error_result,
				/** @param {unknown} e */
				(e) => invoke_error_boundary(e, this.#effect && this.#effect.parent)
			);
			else handle_error_result(result);
		});
	}
};
/**
* @param {Element} text
* @param {string} value
* @returns {void}
*/
function set_text(text, value) {
	var str = value == null ? "" : typeof value === "object" ? `${value}` : value;
	if (str !== (text[TEXT_CACHE] ??= text.nodeValue)) {
		/** @type {any} */ text[TEXT_CACHE] = str;
		text.nodeValue = `${str}`;
	}
}
/**
* Mounts a component to the given target and returns the exports and potentially the props (if compiled with `accessors: true`) of the component.
* Transitions will play during the initial render unless the `intro` option is set to `false`.
*
* @template {Record<string, any>} Props
* @template {Record<string, any>} Exports
* @param {ComponentType<SvelteComponent<Props>> | Component<Props, Exports, any>} component
* @param {MountOptions<Props>} options
* @returns {Exports}
*/
function mount(component, options) {
	return _mount(component, options);
}
/** @type {Map<EventTarget, Map<string, number>>} */
var listeners$1 = /* @__PURE__ */ new Map();
/**
* @template {Record<string, any>} Exports
* @param {ComponentType<SvelteComponent<any>> | Component<any>} Component
* @param {MountOptions} options
* @returns {Exports}
*/
function _mount(Component, { target, anchor, props = {}, events, context, intro = true, transformError }) {
	init_operations();
	/** @type {Exports} */
	var component = void 0;
	var unmount = component_root(() => {
		var anchor_node = anchor ?? target.appendChild(create_text());
		boundary(anchor_node, { pending: () => {} }, (anchor_node) => {
			push({});
			var ctx = component_context;
			if (context) ctx.c = context;
			if (events)
 /** @type {any} */ props.$$events = events;
			if (hydrating) assign_nodes(anchor_node, null);
			component = Component(anchor_node, props) || mark_as_component();
			if (hydrating) {
				/** @type {Effect & { nodes: EffectNodes }} */ active_effect.nodes.end = hydrate_node;
				if (hydrate_node === null || hydrate_node.nodeType !== 8 || hydrate_node.data !== "]") {
					hydration_mismatch();
					throw HYDRATION_ERROR;
				}
			}
			pop();
		}, transformError);
		/** @type {Set<string>} */
		var registered_events = /* @__PURE__ */ new Set();
		/** @param {Array<string>} events */
		var event_handle = (events) => {
			for (var i = 0; i < events.length; i++) {
				var event_name = events[i];
				if (registered_events.has(event_name)) continue;
				registered_events.add(event_name);
				var passive = is_passive_event(event_name);
				for (const node of [target, document]) {
					var counts = listeners$1.get(node);
					if (counts === void 0) {
						counts = /* @__PURE__ */ new Map();
						listeners$1.set(node, counts);
					}
					var count = counts.get(event_name);
					if (count === void 0) {
						node.addEventListener(event_name, handle_event_propagation, { passive });
						counts.set(event_name, 1);
					} else counts.set(event_name, count + 1);
				}
			}
		};
		event_handle(array_from(all_registered_events));
		root_event_handles.add(event_handle);
		return () => {
			for (var event_name of registered_events) for (const node of [target, document]) {
				var counts = listeners$1.get(node);
				var count = counts.get(event_name);
				if (--count == 0) {
					node.removeEventListener(event_name, handle_event_propagation);
					counts.delete(event_name);
					if (counts.size === 0) listeners$1.delete(node);
				} else counts.set(event_name, count);
			}
			root_event_handles.delete(event_handle);
			if (anchor_node !== anchor) anchor_node.parentNode?.removeChild(anchor_node);
		};
	});
	mounted_components.set(component, unmount);
	return component;
}
/**
* References of the components that were mounted or hydrated.
* Uses a `WeakMap` to avoid memory leaks.
*/
var mounted_components = /* @__PURE__ */ new WeakMap();
//#endregion
//#region node_modules/svelte/src/internal/client/dom/blocks/branches.js
/** @import { Effect, TemplateNode } from '#client' */
/**
* @typedef {{ effect: Effect, fragment: DocumentFragment }} Branch
*/
/**
* @template Key
*/
var BranchManager = class {
	/** @type {TemplateNode} */
	anchor;
	/** @type {Map<Batch, Key>} */
	#batches = /* @__PURE__ */ new Map();
	/**
	* Map of keys to effects that are currently rendered in the DOM.
	* These effects are visible and actively part of the document tree.
	* Example:
	* ```
	* {#if condition}
	* 	foo
	* {:else}
	* 	bar
	* {/if}
	* ```
	* Can result in the entries `true->Effect` and `false->Effect`
	* @type {Map<Key, Effect>}
	*/
	#onscreen = /* @__PURE__ */ new Map();
	/**
	* Similar to #onscreen with respect to the keys, but contains branches that are not yet
	* in the DOM, because their insertion is deferred.
	* @type {Map<Key, Branch>}
	*/
	#offscreen = /* @__PURE__ */ new Map();
	/**
	* Keys of effects that are currently outroing
	* @type {Set<Key>}
	*/
	#outroing = /* @__PURE__ */ new Set();
	/**
	* Whether to pause (i.e. outro) on change, or destroy immediately.
	* This is necessary for `<svelte:element>`
	*/
	#transition = true;
	/**
	* @param {TemplateNode} anchor
	* @param {boolean} transition
	*/
	constructor(anchor, transition = true) {
		this.anchor = anchor;
		this.#transition = transition;
	}
	/**
	* @param {Batch} batch
	*/
	#commit = (batch) => {
		if (!this.#batches.has(batch)) return;
		var key = this.#batches.get(batch);
		var onscreen = this.#onscreen.get(key);
		if (onscreen) {
			resume_effect(onscreen);
			this.#outroing.delete(key);
		} else {
			var offscreen = this.#offscreen.get(key);
			if (offscreen) {
				resume_effect(offscreen.effect);
				this.#onscreen.set(key, offscreen.effect);
				this.#offscreen.delete(key);
				/** @type {TemplateNode} */ offscreen.fragment.lastChild.remove();
				this.anchor.before(offscreen.fragment);
				onscreen = offscreen.effect;
			}
		}
		for (const [b, k] of this.#batches) {
			this.#batches.delete(b);
			if (b === batch) break;
			const offscreen = this.#offscreen.get(k);
			if (offscreen) {
				destroy_effect(offscreen.effect);
				this.#offscreen.delete(k);
			}
		}
		for (const [k, effect] of this.#onscreen) {
			if (k === key || this.#outroing.has(k)) continue;
			const on_destroy = () => {
				if (Array.from(this.#batches.values()).includes(k)) {
					var fragment = document.createDocumentFragment();
					move_effect(effect, fragment);
					fragment.append(create_text());
					this.#offscreen.set(k, {
						effect,
						fragment
					});
				} else destroy_effect(effect);
				this.#outroing.delete(k);
				this.#onscreen.delete(k);
			};
			if (this.#transition || !onscreen) {
				this.#outroing.add(k);
				pause_effect(effect, on_destroy, false);
			} else on_destroy();
		}
	};
	/**
	* @param {Batch} batch
	*/
	#discard = (batch) => {
		this.#batches.delete(batch);
		const keys = Array.from(this.#batches.values());
		for (const [k, branch] of this.#offscreen) if (!keys.includes(k)) {
			destroy_effect(branch.effect);
			this.#offscreen.delete(k);
		}
	};
	/**
	*
	* @param {any} key
	* @param {null | ((target: TemplateNode) => void)} fn
	*/
	ensure(key, fn) {
		var batch = current_batch;
		var defer = should_defer_append();
		if (fn && !this.#onscreen.has(key) && !this.#offscreen.has(key)) {
			if (defer) {
				var fragment = document.createDocumentFragment();
				var target = create_text();
				fragment.append(target);
				this.#offscreen.set(key, {
					effect: branch(() => fn(target)),
					fragment
				});
			} else this.#onscreen.set(key, branch(() => fn(this.anchor)));
		}
		this.#batches.set(batch, key);
		if (defer) {
			for (const [k, effect] of this.#onscreen) if (k === key) batch.unskip_effect(effect);
			else batch.skip_effect(effect);
			for (const [k, branch] of this.#offscreen) if (k === key) batch.unskip_effect(branch.effect);
			else batch.skip_effect(branch.effect);
			batch.oncommit(this.#commit);
			batch.ondiscard(this.#discard);
		} else {
			if (hydrating) this.anchor = hydrate_node;
			this.#commit(batch);
		}
	}
};
//#endregion
//#region node_modules/svelte/src/internal/client/dom/blocks/if.js
/** @import { TemplateNode } from '#client' */
/**
* @param {TemplateNode} node
* @param {(branch: (fn: (anchor: Node) => void, key?: number | false) => void) => void} fn
* @param {boolean} [elseif] True if this is an `{:else if ...}` block rather than an `{#if ...}`, as that affects which transitions are considered 'local'
* @returns {void}
*/
function if_block(node, fn, elseif = false) {
	/** @type {TemplateNode | undefined} */
	var marker;
	if (hydrating) {
		marker = hydrate_node;
		hydrate_next();
	}
	var branches = new BranchManager(node);
	var flags = elseif ? EFFECT_TRANSPARENT : 0;
	/**
	* @param {number | false} key
	* @param {null | ((anchor: Node) => void)} fn
	*/
	function update_branch(key, fn) {
		if (hydrating) {
			var data = read_hydration_instruction(marker);
			if (key !== parseInt(data.substring(1))) {
				var anchor = skip_nodes();
				set_hydrate_node(anchor);
				branches.anchor = anchor;
				set_hydrating(false);
				branches.ensure(key, fn);
				set_hydrating(true);
				return;
			}
		}
		branches.ensure(key, fn);
	}
	block(() => {
		var has_branch = false;
		fn((fn, key = 0) => {
			has_branch = true;
			update_branch(key, fn);
		});
		if (!has_branch) update_branch(-1, null);
	}, flags);
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/blocks/key.js
/** @import { TemplateNode } from '#client' */
var NAN = Symbol("NaN");
/**
* @template V
* @param {TemplateNode} node
* @param {() => V} get_key
* @param {(anchor: Node) => TemplateNode | void} render_fn
* @returns {void}
*/
function key$1(node, get_key, render_fn) {
	if (hydrating) hydrate_next();
	var branches = new BranchManager(node);
	var legacy = !is_runes();
	block(() => {
		var key = get_key();
		if (key !== key) key = NAN;
		if (legacy && key !== null && typeof key === "object") key = {};
		branches.ensure(key, render_fn);
	});
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/blocks/each.js
/** @import { EachItem, EachOutroGroup, EachState, Effect, EffectNodes, MaybeSource, Source, TemplateNode, TransitionManager, Value } from '#client' */
/** @import { Batch } from '../../reactivity/batch.js'; */
/**
* @param {any} _
* @param {number} i
*/
function index(_, i) {
	return i;
}
/**
* Pause multiple effects simultaneously, and coordinate their
* subsequent destruction. Used in each blocks
* @param {EachState} state
* @param {Effect[]} to_destroy
* @param {null | Node} controlled_anchor
*/
function pause_effects(state, to_destroy, controlled_anchor) {
	/** @type {TransitionManager[]} */
	var transitions = [];
	var length = to_destroy.length;
	/** @type {EachOutroGroup} */
	var group;
	var remaining = to_destroy.length;
	for (var i = 0; i < length; i++) {
		let effect = to_destroy[i];
		pause_effect(effect, () => {
			if (group) {
				group.pending.delete(effect);
				group.done.add(effect);
				if (group.pending.size === 0) {
					var groups = state.outrogroups;
					destroy_effects(state, array_from(group.done));
					groups.delete(group);
					if (groups.size === 0) state.outrogroups = null;
				}
			} else remaining -= 1;
		}, false);
	}
	if (remaining === 0) {
		var fast_path = transitions.length === 0 && controlled_anchor !== null && state.pending.size === 0;
		if (fast_path) {
			var anchor = controlled_anchor;
			var parent_node = anchor.parentNode;
			clear_text_content(parent_node);
			parent_node.append(anchor);
			state.items.clear();
		}
		destroy_effects(state, to_destroy, !fast_path);
	} else {
		group = {
			pending: new Set(to_destroy),
			done: /* @__PURE__ */ new Set()
		};
		(state.outrogroups ??= /* @__PURE__ */ new Set()).add(group);
	}
}
/**
* @param {EachState} state
* @param {Effect[]} to_destroy
* @param {boolean} remove_dom
*/
function destroy_effects(state, to_destroy, remove_dom = true) {
	/** @type {Set<Effect> | undefined} */
	var preserved_effects;
	if (state.pending.size > 0) {
		preserved_effects = /* @__PURE__ */ new Set();
		for (const keys of state.pending.values()) for (const key of keys) preserved_effects.add(
			/** @type {EachItem} */
			state.items.get(key).e
		);
	}
	for (var i = 0; i < to_destroy.length; i++) {
		var e = to_destroy[i];
		if (preserved_effects?.has(e)) {
			e.f |= EFFECT_OFFSCREEN;
			move_effect(e, document.createDocumentFragment());
		} else destroy_effect(to_destroy[i], remove_dom);
	}
}
/** @type {TemplateNode} */
var offscreen_anchor;
/**
* @template V
* @param {Element | Comment} node The next sibling node, or the parent node if this is a 'controlled' block
* @param {number} flags
* @param {() => V[]} get_collection
* @param {(value: V, index: number) => any} get_key
* @param {(anchor: Node, item: MaybeSource<V>, index: MaybeSource<number>) => void} render_fn
* @param {null | ((anchor: Node) => void)} fallback_fn
* @returns {void}
*/
function each(node, flags, get_collection, get_key, render_fn, fallback_fn = null) {
	var anchor = node;
	/** @type {Map<any, EachItem>} */
	var items = /* @__PURE__ */ new Map();
	if ((flags & 4) !== 0) {
		var parent_node = node;
		anchor = hydrating ? set_hydrate_node(/* @__PURE__ */ get_first_child(parent_node)) : parent_node.appendChild(create_text());
	}
	if (hydrating) hydrate_next();
	/** @type {Effect | null} */
	var fallback = null;
	var each_array = /* @__PURE__ */ derived_safe_equal(() => {
		var collection = get_collection();
		return is_array(collection) ? collection : collection == null ? [] : array_from(collection);
	});
	/** @type {V[]} */
	var array;
	/** @type {Map<Batch, Set<any>>} */
	var pending = /* @__PURE__ */ new Map();
	var first_run = true;
	/**
	* @param {Batch} batch
	*/
	function commit(batch) {
		if ((state.effect.f & 16384) !== 0) return;
		state.pending.delete(batch);
		state.fallback = fallback;
		reconcile(state, array, anchor, flags, get_key);
		if (fallback !== null) {
			if (array.length === 0) {
				if ((fallback.f & 33554432) === 0) resume_effect(fallback);
				else {
					fallback.f ^= EFFECT_OFFSCREEN;
					move(fallback, null, anchor);
				}
			} else pause_effect(fallback, () => {
				fallback = null;
			});
		}
	}
	/**
	* @param {Batch} batch
	*/
	function discard(batch) {
		state.pending.delete(batch);
	}
	/** @type {EachState} */
	var state = {
		effect: block(() => {
			array = get(each_array);
			var length = array.length;
			/** `true` if there was a hydration mismatch. Needs to be a `let` or else it isn't treeshaken out */
			let mismatch = false;
			if (hydrating) {
				if (read_hydration_instruction(anchor) === "[!" !== (length === 0)) {
					anchor = skip_nodes();
					set_hydrate_node(anchor);
					set_hydrating(false);
					mismatch = true;
				}
			}
			var keys = /* @__PURE__ */ new Set();
			var batch = current_batch;
			var defer = should_defer_append();
			for (var index = 0; index < length; index += 1) {
				if (hydrating && hydrate_node.nodeType === 8 && hydrate_node.data === "]") {
					anchor = hydrate_node;
					mismatch = true;
					set_hydrating(false);
				}
				var value = array[index];
				var key = get_key(value, index);
				var item = first_run ? null : items.get(key);
				if (item) {
					if (item.v) internal_set(item.v, value);
					if (item.i) internal_set(item.i, index);
					if (defer) batch.unskip_effect(item.e);
				} else {
					item = create_item(items, first_run ? anchor : offscreen_anchor ??= create_text(), value, key, index, render_fn, flags, get_collection);
					if (!first_run) item.e.f |= EFFECT_OFFSCREEN;
					items.set(key, item);
				}
				keys.add(key);
			}
			if (length === 0 && fallback_fn && !fallback) {
				if (first_run) fallback = branch(() => fallback_fn(anchor));
				else {
					fallback = branch(() => fallback_fn(offscreen_anchor ??= create_text()));
					fallback.f |= EFFECT_OFFSCREEN;
				}
			}
			if (length > keys.size) each_key_duplicate("", "", "");
			if (hydrating && length > 0) set_hydrate_node(skip_nodes());
			if (!first_run) {
				pending.set(batch, keys);
				if (defer) {
					for (const [key, item] of items) if (!keys.has(key)) batch.skip_effect(item.e);
					batch.oncommit(commit);
					batch.ondiscard(discard);
				} else commit(batch);
			}
			if (mismatch) set_hydrating(true);
			get(each_array);
		}),
		flags,
		items,
		pending,
		outrogroups: null,
		fallback
	};
	first_run = false;
	if (hydrating) anchor = hydrate_node;
}
/**
* Skip past any non-branch effects (which could be created with `createSubscriber`, for example) to find the next branch effect
* @param {Effect | null} effect
* @returns {Effect | null}
*/
function skip_to_branch(effect) {
	while (effect !== null && (effect.f & 32) === 0) effect = effect.next;
	return effect;
}
/**
* Add, remove, or reorder items output by an each block as its input changes
* @template V
* @param {EachState} state
* @param {Array<V>} array
* @param {Element | Comment | Text} anchor
* @param {number} flags
* @param {(value: V, index: number) => any} get_key
* @returns {void}
*/
function reconcile(state, array, anchor, flags, get_key) {
	var is_animated = (flags & 8) !== 0;
	var length = array.length;
	var items = state.items;
	var current = skip_to_branch(state.effect.first);
	/** @type {undefined | Set<Effect>} */
	var seen;
	/** @type {Effect | null} */
	var prev = null;
	/** @type {undefined | Set<Effect>} */
	var to_animate;
	/** @type {Effect[]} */
	var matched = [];
	/** @type {Effect[]} */
	var stashed = [];
	/** @type {V} */
	var value;
	/** @type {any} */
	var key;
	/** @type {Effect | undefined} */
	var effect;
	/** @type {number} */
	var i;
	if (is_animated) for (i = 0; i < length; i += 1) {
		value = array[i];
		key = get_key(value, i);
		effect = items.get(key).e;
		if ((effect.f & 33554432) === 0) {
			effect.nodes?.a?.measure();
			(to_animate ??= /* @__PURE__ */ new Set()).add(effect);
		}
	}
	for (i = 0; i < length; i += 1) {
		value = array[i];
		key = get_key(value, i);
		effect = items.get(key).e;
		if (state.outrogroups !== null) for (const group of state.outrogroups) {
			group.pending.delete(effect);
			group.done.delete(effect);
		}
		if ((effect.f & 8192) !== 0) {
			resume_effect(effect);
			if (is_animated) {
				effect.nodes?.a?.unfix();
				(to_animate ??= /* @__PURE__ */ new Set()).delete(effect);
			}
		}
		if ((effect.f & 33554432) !== 0) {
			effect.f ^= EFFECT_OFFSCREEN;
			if (effect === current) move(effect, null, anchor);
			else {
				var next = prev ? prev.next : current;
				if (effect === state.effect.last) state.effect.last = effect.prev;
				if (effect.prev) effect.prev.next = effect.next;
				if (effect.next) effect.next.prev = effect.prev;
				link(state, prev, effect);
				link(state, effect, next);
				move(effect, next, anchor);
				prev = effect;
				matched = [];
				stashed = [];
				current = skip_to_branch(prev.next);
				continue;
			}
		}
		if (effect !== current) {
			if (seen !== void 0 && seen.has(effect)) {
				if (matched.length < stashed.length) {
					var start = stashed[0];
					var j;
					prev = start.prev;
					var a = matched[0];
					var b = matched[matched.length - 1];
					for (j = 0; j < matched.length; j += 1) move(matched[j], start, anchor);
					for (j = 0; j < stashed.length; j += 1) seen.delete(stashed[j]);
					link(state, a.prev, b.next);
					link(state, prev, a);
					link(state, b, start);
					current = start;
					prev = b;
					i -= 1;
					matched = [];
					stashed = [];
				} else {
					seen.delete(effect);
					move(effect, current, anchor);
					link(state, effect.prev, effect.next);
					link(state, effect, prev === null ? state.effect.first : prev.next);
					link(state, prev, effect);
					prev = effect;
				}
				continue;
			}
			matched = [];
			stashed = [];
			while (current !== null && current !== effect) {
				(seen ??= /* @__PURE__ */ new Set()).add(current);
				stashed.push(current);
				current = skip_to_branch(current.next);
			}
			if (current === null) continue;
		}
		if ((effect.f & 33554432) === 0) matched.push(effect);
		prev = effect;
		current = skip_to_branch(effect.next);
	}
	if (state.outrogroups !== null) {
		for (const group of state.outrogroups) if (group.pending.size === 0) {
			destroy_effects(state, array_from(group.done));
			state.outrogroups?.delete(group);
		}
		if (state.outrogroups.size === 0) state.outrogroups = null;
	}
	if (current !== null || seen !== void 0) {
		/** @type {Effect[]} */
		var to_destroy = [];
		if (seen !== void 0) {
			for (effect of seen) if ((effect.f & 8192) === 0) to_destroy.push(effect);
		}
		while (current !== null) {
			if ((current.f & 8192) === 0 && current !== state.fallback) to_destroy.push(current);
			current = skip_to_branch(current.next);
		}
		var destroy_length = to_destroy.length;
		if (destroy_length > 0) {
			var controlled_anchor = (flags & 4) !== 0 && length === 0 ? anchor : null;
			if (is_animated) {
				for (i = 0; i < destroy_length; i += 1) to_destroy[i].nodes?.a?.measure();
				for (i = 0; i < destroy_length; i += 1) to_destroy[i].nodes?.a?.fix();
			}
			pause_effects(state, to_destroy, controlled_anchor);
		}
	}
	if (is_animated) queue_micro_task(() => {
		if (to_animate === void 0) return;
		for (effect of to_animate) effect.nodes?.a?.apply();
	});
}
/**
* @template V
* @param {Map<any, EachItem>} items
* @param {Node} anchor
* @param {V} value
* @param {unknown} key
* @param {number} index
* @param {(anchor: Node, item: V | Source<V>, index: number | Value<number>, collection: () => V[]) => void} render_fn
* @param {number} flags
* @param {() => V[]} get_collection
* @returns {EachItem}
*/
function create_item(items, anchor, value, key, index, render_fn, flags, get_collection) {
	var v = (flags & 1) !== 0 ? (flags & 16) === 0 ? /* @__PURE__ */ mutable_source(value, false, false) : source(value) : null;
	var i = (flags & 2) !== 0 ? source(index) : null;
	return {
		v,
		i,
		e: branch(() => {
			render_fn(anchor, v ?? value, i ?? index, get_collection);
			return () => {
				items.delete(key);
			};
		})
	};
}
/**
* @param {Effect} effect
* @param {Effect | null} next
* @param {Text | Element | Comment} anchor
*/
function move(effect, next, anchor) {
	if (!effect.nodes) return;
	var node = effect.nodes.start;
	var end = effect.nodes.end;
	var dest = next && (next.f & 33554432) === 0 ? next.nodes.start : anchor;
	while (node !== null) {
		var next_node = /* @__PURE__ */ get_next_sibling(node);
		dest.before(node);
		if (node === end) return;
		node = next_node;
	}
}
/**
* @param {EachState} state
* @param {Effect | null} prev
* @param {Effect | null} next
*/
function link(state, prev, next) {
	if (prev === null) state.effect.first = next;
	else prev.next = next;
	if (next === null) state.effect.last = prev;
	else next.prev = prev;
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/blocks/snippet.js
/** @import { Snippet } from 'svelte' */
/** @import { TemplateNode } from '#client' */
/** @import { Getters } from '#shared' */
/**
* @template {(node: TemplateNode, ...args: any[]) => void} SnippetFn
* @param {TemplateNode} node
* @param {() => SnippetFn | null | undefined} get_snippet
* @param {(() => any)[]} args
* @returns {void}
*/
function snippet(node, get_snippet, ...args) {
	var branches = new BranchManager(node);
	block(() => {
		const snippet = get_snippet() ?? null;
		branches.ensure(snippet, snippet && ((anchor) => snippet(anchor, ...args)));
	}, EFFECT_TRANSPARENT);
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/blocks/svelte-component.js
/** @import { TemplateNode, Dom } from '#client' */
/**
* @template P
* @template {(props: P) => void} C
* @param {TemplateNode} node
* @param {() => C} get_component
* @param {(anchor: TemplateNode, component: C) => Dom | void} render_fn
* @returns {void}
*/
function component(node, get_component, render_fn) {
	/** @type {TemplateNode | undefined} */
	var hydration_start_node;
	if (hydrating) {
		hydration_start_node = hydrate_node;
		hydrate_next();
	}
	var branches = new BranchManager(node);
	block(() => {
		var component = get_component() ?? null;
		if (hydrating) {
			if (read_hydration_instruction(hydration_start_node) === "[" !== (component !== null)) {
				var anchor = skip_nodes();
				set_hydrate_node(anchor);
				branches.anchor = anchor;
				set_hydrating(false);
				branches.ensure(component, component && ((target) => render_fn(target, component)));
				set_hydrating(true);
				return;
			}
		}
		branches.ensure(component, component && ((target) => render_fn(target, component)));
	}, EFFECT_TRANSPARENT);
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/elements/actions.js
/** @import { ActionPayload } from '#client' */
/**
* @template P
* @param {Element} dom
* @param {(dom: Element, value?: P) => ActionPayload<P>} action
* @param {() => P} [get_value]
* @returns {void}
*/
function action(dom, action, get_value) {
	effect(() => {
		var payload = untrack(() => action(dom, get_value?.()) || {});
		if (get_value && payload?.update) {
			var inited = false;
			/** @type {P} */
			var prev = {};
			render_effect(() => {
				var value = get_value();
				deep_read_state(value);
				if (inited && safe_not_equal(prev, value)) {
					prev = value;
					/** @type {Function} */ payload.update(value);
				}
			});
			inited = true;
		}
		if (payload?.destroy) return () => payload.destroy();
	});
}
//#endregion
//#region node_modules/clsx/dist/clsx.mjs
function r(e) {
	var t, f, n = "";
	if ("string" == typeof e || "number" == typeof e) n += e;
	else if ("object" == typeof e) if (Array.isArray(e)) {
		var o = e.length;
		for (t = 0; t < o; t++) e[t] && (f = r(e[t])) && (n && (n += " "), n += f);
	} else for (f in e) e[f] && (n && (n += " "), n += f);
	return n;
}
function clsx$1() {
	for (var e, t, f = 0, n = "", o = arguments.length; f < o; f++) (e = arguments[f]) && (t = r(e)) && (n && (n += " "), n += t);
	return n;
}
//#endregion
//#region node_modules/svelte/src/internal/shared/attributes.js
/**
* Small wrapper around clsx to preserve Svelte's (weird) handling of falsy values.
* TODO Svelte 6 revisit this, and likely turn all falsy values into the empty string (what clsx also does)
* @param  {any} value
*/
function clsx(value) {
	if (typeof value === "object") return clsx$1(value);
	else return value ?? "";
}
var whitespace = [..." 	\n\r\f\xA0\v﻿"];
/**
* @param {any} value
* @param {string | null} [hash]
* @param {Record<string, boolean>} [directives]
* @returns {string | null}
*/
function to_class(value, hash, directives) {
	var classname = value == null ? "" : "" + value;
	if (hash) classname = classname ? classname + " " + hash : hash;
	if (directives) {
		for (var key of Object.keys(directives)) if (directives[key]) classname = classname ? classname + " " + key : key;
		else if (classname.length) {
			var len = key.length;
			var a = 0;
			while ((a = classname.indexOf(key, a)) >= 0) {
				var b = a + len;
				if ((a === 0 || whitespace.includes(classname[a - 1])) && (b === classname.length || whitespace.includes(classname[b]))) classname = (a === 0 ? "" : classname.substring(0, a)) + classname.substring(b + 1);
				else a = b;
			}
		}
	}
	return classname === "" ? null : classname;
}
/**
*
* @param {Record<string,any>} styles
* @param {boolean} important
*/
function append_styles(styles, important = false) {
	var separator = important ? " !important;" : ";";
	var css = "";
	for (var key of Object.keys(styles)) {
		var value = styles[key];
		if (value != null && value !== "") css += " " + key + ": " + value + separator;
	}
	return css;
}
/**
* @param {string} name
* @returns {string}
*/
function to_css_name(name) {
	if (name[0] !== "-" || name[1] !== "-") return name.toLowerCase();
	return name;
}
/**
* @param {any} value
* @param {Record<string, any> | [Record<string, any>, Record<string, any>]} [styles]
* @returns {string | null}
*/
function to_style(value, styles) {
	if (styles) {
		var new_style = "";
		/** @type {Record<string,any> | undefined} */
		var normal_styles;
		/** @type {Record<string,any> | undefined} */
		var important_styles;
		if (Array.isArray(styles)) {
			normal_styles = styles[0];
			important_styles = styles[1];
		} else normal_styles = styles;
		if (value) {
			value = String(value).replaceAll(/\/\*.*?\*\//g, "").trim();
			/** @type {boolean | '"' | "'"} */
			var in_str = false;
			var in_apo = 0;
			var in_comment = false;
			var reserved_names = [];
			if (normal_styles) reserved_names.push(...Object.keys(normal_styles).map(to_css_name));
			if (important_styles) reserved_names.push(...Object.keys(important_styles).map(to_css_name));
			var start_index = 0;
			var name_index = -1;
			const len = value.length;
			for (var i = 0; i < len; i++) {
				var c = value[i];
				if (in_comment) {
					if (c === "/" && value[i - 1] === "*") in_comment = false;
				} else if (in_str) {
					if (in_str === c) in_str = false;
				} else if (c === "/" && value[i + 1] === "*") in_comment = true;
				else if (c === "\"" || c === "'") in_str = c;
				else if (c === "(") in_apo++;
				else if (c === ")") in_apo--;
				if (!in_comment && in_str === false && in_apo === 0) {
					if (c === ":" && name_index === -1) name_index = i;
					else if (c === ";" || i === len - 1) {
						if (name_index !== -1) {
							var name = to_css_name(value.substring(start_index, name_index).trim());
							if (!reserved_names.includes(name)) {
								if (c !== ";") i++;
								var property = value.substring(start_index, i).trim();
								new_style += " " + property + ";";
							}
						}
						start_index = i + 1;
						name_index = -1;
					}
				}
			}
		}
		if (normal_styles) new_style += append_styles(normal_styles);
		if (important_styles) new_style += append_styles(important_styles, true);
		new_style = new_style.trim();
		return new_style === "" ? null : new_style;
	}
	return value == null ? null : String(value);
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/elements/class.js
/**
* @param {Element} dom
* @param {boolean | number} is_html
* @param {string | null} value
* @param {string} [hash]
* @param {Record<string, any>} [prev_classes]
* @param {Record<string, any>} [next_classes]
* @returns {Record<string, boolean> | undefined}
*/
function set_class(dom, is_html, value, hash, prev_classes, next_classes) {
	var prev = dom[CLASS_CACHE];
	if (hydrating || prev !== value || prev === void 0) {
		var next_class_name = to_class(value, hash, next_classes);
		if (!hydrating || next_class_name !== dom.getAttribute("class")) {
			if (next_class_name == null) dom.removeAttribute("class");
			else if (is_html) dom.className = next_class_name;
			else dom.setAttribute("class", next_class_name);
		}
		/** @type {any} */ dom[CLASS_CACHE] = value;
	} else if (next_classes && prev_classes !== next_classes) for (var key in next_classes) {
		var is_present = !!next_classes[key];
		if (prev_classes == null || is_present !== !!prev_classes[key]) dom.classList.toggle(key, is_present);
	}
	return next_classes;
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/elements/style.js
/**
* @param {Element & ElementCSSInlineStyle} dom
* @param {Record<string, any>} prev
* @param {Record<string, any>} next
* @param {string} [priority]
*/
function update_styles(dom, prev = {}, next, priority) {
	for (var key in next) {
		var value = next[key];
		if (prev[key] !== value) {
			if (next[key] == null) dom.style.removeProperty(key);
			else dom.style.setProperty(key, value, priority);
		}
	}
}
/**
* @param {Element & ElementCSSInlineStyle} dom
* @param {string | null} value
* @param {Record<string, any> | [Record<string, any>, Record<string, any>]} [prev_styles]
* @param {Record<string, any> | [Record<string, any>, Record<string, any>]} [next_styles]
*/
function set_style(dom, value, prev_styles, next_styles) {
	var prev = dom[STYLE_CACHE];
	if (hydrating || prev !== value) {
		var next_style_attr = to_style(value, next_styles);
		if (!hydrating || next_style_attr !== dom.getAttribute("style")) {
			if (next_style_attr == null) dom.removeAttribute("style");
			else dom.style.cssText = next_style_attr;
		}
		/** @type {any} */ dom[STYLE_CACHE] = value;
	} else if (next_styles) {
		if (Array.isArray(next_styles)) {
			update_styles(dom, prev_styles?.[0], next_styles[0]);
			update_styles(dom, prev_styles?.[1], next_styles[1], "important");
		} else update_styles(dom, prev_styles, next_styles);
	}
	return next_styles;
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/elements/bindings/select.js
/**
* Sets the `selected` attribute on an option so form reset can restore it.
* @param {HTMLOptionElement} option
* @param {boolean} selected
*/
function set_selected(option, selected) {
	if (selected) {
		if (!option.hasAttribute("selected")) option.setAttribute("selected", "");
	} else option.removeAttribute("selected");
}
/**
* Marks the options matching `__defaultValue` as selected. Without `preserve`
* a newly matching option gets selected, as an inserted `<option selected>` would.
* @param {HTMLSelectElement} select
* @param {boolean} preserve
*/
function apply_default_select_value(select, preserve) {
	var value = select.__defaultValue;
	var multiple = select.multiple;
	var values = multiple ? value ?? [] : null;
	if (multiple && !is_array(values)) return;
	var index = select.selectedIndex;
	var selected = preserve && multiple ? new Set(select.selectedOptions) : null;
	for (var option of select.options) {
		var option_value = get_option_value(option);
		set_selected(option, multiple ? values.includes(option_value) : is(option_value, value));
	}
	if (!preserve) return;
	if (selected !== null) for (option of select.options) {
		var was_selected = selected.has(option);
		if (option.selected !== was_selected) option.selected = was_selected;
	}
	else if (select.selectedIndex !== index) select.selectedIndex = index;
}
/**
* Selects the correct option(s) (depending on whether this is a multiple select)
* @template V
* @param {HTMLSelectElement} select
* @param {V} value
* @param {boolean} mounting
*/
function select_option(select, value, mounting = false) {
	if (select.multiple) {
		if (value == void 0) return;
		if (!is_array(value)) return select_multiple_invalid_value();
		for (var option of select.options) option.selected = value.includes(get_option_value(option));
		return;
	}
	for (option of select.options) if (is(get_option_value(option), value)) {
		option.selected = true;
		return;
	}
	if (!mounting || value !== void 0) select.selectedIndex = -1;
}
/**
* Sets up a mutation observer to sync the current selection
* and default to the dom when the options change, for example
* when they are inside an `#each` block. Called once per `<select>`,
* by the compiled output or by `attribute_effect` for spreads.
* @param {HTMLSelectElement} select
*/
function init_select(select) {
	var observer = new MutationObserver((entries) => {
		if (entries.every(is_selectedcontent_mutation)) return;
		if ("__defaultValue" in select) apply_default_select_value(select, false);
		if ("__value" in select) select_option(select, select.__value);
	});
	observer.observe(select, {
		childList: true,
		subtree: true,
		attributes: true,
		attributeFilter: ["value"]
	});
	teardown(() => {
		observer.disconnect();
	});
}
/**
* @param {HTMLSelectElement} select
* @param {() => unknown} get
* @param {(value: unknown) => void} set
* @returns {void}
*/
function bind_select_value(select, get, set = get) {
	var batches = /* @__PURE__ */ new WeakSet();
	var mounting = true;
	listen_to_event_and_reset_event(select, "change", (is_reset) => {
		var query = is_reset ? "[selected]" : ":checked";
		/** @type {unknown} */
		var value;
		if (select.multiple) value = [].map.call(select.querySelectorAll(query), get_option_value);
		else {
			/** @type {HTMLOptionElement | null} */
			var selected_option = select.querySelector(query) ?? select.querySelector("option:not([disabled])");
			value = selected_option && get_option_value(selected_option);
		}
		set(value);
		select.__value = value;
		if (current_batch !== null) batches.add(current_batch);
	});
	effect(() => {
		var value = get();
		if (select === document.activeElement) {
			var batch = async_mode_flag ? previous_batch : current_batch;
			if (batches.has(batch)) return;
		}
		select_option(select, value, mounting);
		if (mounting && value === void 0) {
			/** @type {HTMLOptionElement | null} */
			var selected_option = select.querySelector(":checked");
			if (selected_option !== null) {
				value = get_option_value(selected_option);
				set(value);
			}
		}
		select.__value = value;
		mounting = false;
	});
}
/** @param {HTMLOptionElement} option */
function get_option_value(option) {
	if ("__value" in option) return option.__value;
	else return option.value;
}
/**
* Returns `true` if the mutation stems from the browser mirroring the selected
* option's content into `<selectedcontent>`, or from us replacing the
* `<selectedcontent>` element with a clone of itself
* @param {MutationRecord} entry
*/
function is_selectedcontent_mutation(entry) {
	if (entry.target.closest("selectedcontent") !== null) return true;
	if (entry.type === "childList") {
		var nodes = [...entry.addedNodes, ...entry.removedNodes];
		return nodes.length > 0 && nodes.every((node) => node.nodeName === "SELECTEDCONTENT");
	}
	return false;
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/elements/attributes.js
/** @import { Blocker, Effect } from '#client' */
var IS_CUSTOM_ELEMENT = Symbol("is custom element");
var IS_HTML = Symbol("is html");
var LINK_TAG = IS_XHTML ? "link" : "LINK";
/**
* The value/checked attribute in the template actually corresponds to the defaultValue property, so we need
* to remove it upon hydration to avoid a bug when someone resets the form value.
* @param {HTMLInputElement} input
* @returns {void}
*/
function remove_input_defaults(input) {
	if (!hydrating) return;
	var already_removed = false;
	var remove_defaults = () => {
		if (already_removed) return;
		already_removed = true;
		if (input.hasAttribute("value")) {
			var value = input.value;
			set_attribute(input, "value", null);
			input.value = value;
		}
		if (input.hasAttribute("checked")) {
			var checked = input.checked;
			set_attribute(input, "checked", null);
			input.checked = checked;
		}
	};
	/** @type {any} */ input[FORM_RESET_HANDLER] = remove_defaults;
	queue_micro_task(remove_defaults);
	add_form_reset_listener();
}
/**
* @param {Element} element
* @param {string} attribute
* @param {string | null} value
* @param {boolean} [skip_warning]
*/
function set_attribute(element, attribute, value, skip_warning) {
	var attributes = get_attributes(element);
	if (hydrating) {
		attributes[attribute] = element.getAttribute(attribute);
		if (attribute === "src" || attribute === "srcset" || attribute === "href" && element.nodeName === LINK_TAG) {
			if (!skip_warning);
			return;
		}
	}
	if (attributes[attribute] === (attributes[attribute] = value)) return;
	if (attribute === "loading") element[LOADING_ATTR_SYMBOL] = value;
	if (value == null) element.removeAttribute(attribute);
	else if (typeof value !== "string" && get_setters(element).has(attribute)) element[attribute] = value;
	else element.setAttribute(attribute, value);
}
/**
*
* @param {Element} element
*/
function get_attributes(element) {
	return element[ATTRIBUTES_CACHE] ??= {
		[IS_CUSTOM_ELEMENT]: element.nodeName.includes("-"),
		[IS_HTML]: element.namespaceURI === NAMESPACE_HTML
	};
}
/** @type {Map<string, Set<string>>} */
var setters_cache = /* @__PURE__ */ new Map();
/** @param {Element} element */
function get_setters(element) {
	var cache_key = element.getAttribute("is") || element.nodeName;
	var setters = setters_cache.get(cache_key);
	if (setters) return setters;
	setters_cache.set(cache_key, setters = /* @__PURE__ */ new Set());
	var descriptors;
	var proto = element;
	var element_proto = Element.prototype;
	while (element_proto !== proto) {
		descriptors = get_descriptors(proto);
		for (var key in descriptors) if (descriptors[key].set && key !== "innerHTML" && key !== "textContent" && key !== "innerText") setters.add(key);
		proto = get_prototype_of(proto);
	}
	return setters;
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/elements/bindings/input.js
/** @import { Batch } from '../../../reactivity/batch.js' */
/**
* @param {HTMLInputElement} input
* @param {() => unknown} get
* @param {(value: unknown) => void} set
* @returns {void}
*/
function bind_value(input, get, set = get) {
	var batches = /* @__PURE__ */ new WeakSet();
	listen_to_event_and_reset_event(input, "input", async (is_reset) => {
		/** @type {any} */
		var value = is_reset ? input.defaultValue : input.value;
		value = is_numberlike_input(input) ? to_number(value) : value;
		set(value);
		if (current_batch !== null) batches.add(current_batch);
		await tick();
		if (value !== (value = get())) {
			var start = input.selectionStart;
			var end = input.selectionEnd;
			var length = input.value.length;
			input.value = value ?? "";
			if (end !== null) {
				var new_length = input.value.length;
				if (start === end && end === length && new_length > length) {
					input.selectionStart = new_length;
					input.selectionEnd = new_length;
				} else {
					input.selectionStart = start;
					input.selectionEnd = Math.min(end, new_length);
				}
			}
		}
	});
	if (hydrating && input.defaultValue !== input.value || untrack(get) == null && input.value) {
		set(is_numberlike_input(input) ? to_number(input.value) : input.value);
		if (current_batch !== null) batches.add(current_batch);
	}
	render_effect(() => {
		var value = get();
		if (input === document.activeElement) {
			var batch = async_mode_flag ? previous_batch : current_batch;
			if (batches.has(batch)) return;
		}
		if (is_numberlike_input(input) && value === to_number(input.value)) return;
		if (input.type === "date" && !value && !input.value) return;
		if (value !== input.value) input.value = value ?? "";
	});
}
/**
* @param {HTMLInputElement} input
* @param {() => unknown} get
* @param {(value: unknown) => void} set
* @returns {void}
*/
function bind_checked(input, get, set = get) {
	listen_to_event_and_reset_event(input, "change", (is_reset) => {
		set(is_reset ? input.defaultChecked : input.checked);
	});
	if (hydrating && input.defaultChecked !== input.checked || untrack(get) == null) set(input.checked);
	render_effect(() => {
		var value = get();
		input.checked = Boolean(value);
	});
}
/**
* @param {HTMLInputElement} input
*/
function is_numberlike_input(input) {
	var type = input.type;
	return type === "number" || type === "range";
}
/**
* @param {string} value
*/
function to_number(value) {
	return value === "" ? null : +value;
}
//#endregion
//#region node_modules/svelte/src/internal/client/dom/elements/bindings/this.js
/** @import { ComponentContext, Effect } from '#client' */
/**
* @param {any} bound_value
* @param {Element} element_or_component
* @returns {boolean}
*/
function is_bound_this(bound_value, element_or_component) {
	return bound_value === element_or_component || bound_value?.[STATE_SYMBOL] === element_or_component;
}
/**
* @param {any} element_or_component
* @param {(value: unknown, ...parts: unknown[]) => void} update
* @param {(...parts: unknown[]) => unknown} get_value
* @param {() => unknown[]} [get_parts] Set if the this binding is used inside an each block,
* 										returns all the parts of the each block context that are used in the expression
* @returns {void}
*/
function bind_this(element_or_component = mark_as_component(), update, get_value, get_parts) {
	var component_effect = component_context.r;
	var parent = active_effect;
	effect(() => {
		/** @type {unknown[]} */
		var old_parts;
		/** @type {unknown[]} */
		var parts;
		render_effect(() => {
			old_parts = parts;
			parts = get_parts?.() || [];
			untrack(() => {
				if (!is_bound_this(get_value(...parts), element_or_component)) {
					update(element_or_component, ...parts);
					if (old_parts && is_bound_this(get_value(...old_parts), element_or_component)) update(null, ...old_parts);
				}
			});
		});
		return () => {
			let p = parent;
			while (p !== component_effect && p.parent !== null && p.parent.f & 33554432) p = p.parent;
			const teardown = () => {
				if (parts && is_bound_this(get_value(...parts), element_or_component)) update(null, ...parts);
			};
			const original_teardown = p.teardown;
			p.teardown = () => {
				teardown();
				original_teardown?.();
			};
		};
	});
	return element_or_component;
}
//#endregion
//#region node_modules/svelte/src/internal/client/reactivity/store.js
/**
* Whether or not the prop currently being read is a store binding, as in
* `<Child bind:x={$y} />`. If it is, we treat the prop as mutable even in
* runes mode, and skip `binding_property_non_reactive` validation
*/
var is_store_binding = false;
/**
* Returns a tuple that indicates whether `fn()` reads a prop that is a store binding.
* Used to prevent `binding_property_non_reactive` validation false positives and
* ensure that these props are treated as mutable even in runes mode
* @template T
* @param {() => T} fn
* @returns {[T, boolean]}
*/
function capture_store_binding(fn) {
	var previous_is_store_binding = is_store_binding;
	try {
		is_store_binding = false;
		return [fn(), is_store_binding];
	} finally {
		is_store_binding = previous_is_store_binding;
	}
}
//#endregion
//#region node_modules/svelte/src/internal/client/reactivity/props.js
/** @import { Derived, Effect, Source } from './types.js' */
/**
* The proxy handler for spread props. Handles the incoming array of props
* that looks like `() => { dynamic: props }, { static: prop }, ..` and wraps
* them so that the whole thing is passed to the component as the `$$props` argument.
* @type {ProxyHandler<{ props: Array<Record<string | symbol, unknown> | (() => Record<string | symbol, unknown>)> }>}}
*/
var spread_props_handler = {
	get(target, key) {
		let i = target.props.length;
		while (i--) {
			let p = target.props[i];
			if (is_function(p)) p = p();
			if (typeof p === "object" && p !== null && key in p) return p[key];
		}
	},
	set(target, key, value) {
		let i = target.props.length;
		while (i--) {
			let p = target.props[i];
			if (is_function(p)) p = p();
			const desc = get_descriptor(p, key);
			if (desc && desc.set) {
				desc.set(value);
				return true;
			}
		}
		return false;
	},
	getOwnPropertyDescriptor(target, key) {
		let i = target.props.length;
		while (i--) {
			let p = target.props[i];
			if (is_function(p)) p = p();
			if (typeof p === "object" && p !== null && key in p) {
				const descriptor = get_descriptor(p, key);
				if (descriptor && !descriptor.configurable) descriptor.configurable = true;
				return descriptor;
			}
		}
	},
	has(target, key) {
		if (key === STATE_SYMBOL || key === LEGACY_PROPS) return false;
		for (let p of target.props) {
			if (is_function(p)) p = p();
			if (p != null && key in p) return true;
		}
		return false;
	},
	ownKeys(target) {
		/** @type {Array<string | symbol>} */
		const keys = [];
		for (let p of target.props) {
			if (is_function(p)) p = p();
			if (!p) continue;
			for (const key in p) if (!keys.includes(key)) keys.push(key);
			for (const key of Object.getOwnPropertySymbols(p)) if (!keys.includes(key)) keys.push(key);
		}
		return keys;
	}
};
/**
* @param {Array<Record<string, unknown> | (() => Record<string, unknown>)>} props
* @returns {any}
*/
function spread_props(...props) {
	return new Proxy({ props }, spread_props_handler);
}
/**
* This function is responsible for synchronizing a possibly bound prop with the inner component state.
* It is used whenever the compiler sees that the component writes to the prop, or when it has a default prop_value.
* @template V
* @param {Record<string, unknown>} props
* @param {string} key
* @param {number} flags
* @param {V | (() => V)} [fallback]
* @returns {(() => V | ((arg: V) => V) | ((arg: V, mutation: boolean) => V))}
*/
function prop(props, key, flags, fallback) {
	var runes = !legacy_mode_flag || (flags & 2) !== 0;
	var bindable = (flags & 8) !== 0;
	var lazy = (flags & 16) !== 0;
	var fallback_value = fallback;
	var fallback_dirty = true;
	var fallback_signal = void 0;
	var get_fallback = () => {
		if (lazy && runes) {
			fallback_signal ??= /* @__PURE__ */ derived(fallback);
			return get(fallback_signal);
		}
		if (fallback_dirty) {
			fallback_dirty = false;
			fallback_value = lazy ? untrack(fallback) : fallback;
		}
		return fallback_value;
	};
	/** @type {((v: V) => void) | undefined} */
	let setter;
	if (bindable) {
		var is_entry_props = STATE_SYMBOL in props || LEGACY_PROPS in props;
		setter = get_descriptor(props, key)?.set ?? (is_entry_props && key in props ? (v) => props[key] = v : void 0);
	}
	/** @type {V} */
	var initial_value;
	var is_store_sub = false;
	if (bindable) [initial_value, is_store_sub] = capture_store_binding(() => props[key]);
	else initial_value = props[key];
	if (initial_value === void 0 && fallback !== void 0) {
		initial_value = get_fallback();
		if (setter) {
			if (runes) props_invalid_value(key);
			setter(initial_value);
		}
	}
	/** @type {() => V} */
	var getter;
	if (runes) getter = () => {
		var value = props[key];
		if (value === void 0) return get_fallback();
		fallback_dirty = true;
		return value;
	};
	else getter = () => {
		var value = props[key];
		if (value !== void 0) fallback_value = void 0;
		return value === void 0 ? fallback_value : value;
	};
	if (runes && (flags & 4) === 0) return getter;
	if (setter) {
		var legacy_parent = props.$$legacy;
		return (function(value, mutation) {
			if (arguments.length > 0) {
				if (!runes || !mutation || legacy_parent || is_store_sub)
 /** @type {Function} */ setter(mutation ? getter() : value);
				return value;
			}
			return getter();
		});
	}
	var overridden = false;
	var d = ((flags & 1) !== 0 ? derived : derived_safe_equal)(() => {
		overridden = false;
		return getter();
	});
	if (bindable) get(d);
	var parent_effect = active_effect;
	return (function(value, mutation) {
		if (arguments.length > 0) {
			const new_value = mutation ? get(d) : runes && bindable ? proxy(value) : value;
			set(d, new_value);
			overridden = true;
			if (fallback_value !== void 0) fallback_value = new_value;
			return value;
		}
		if (is_destroying_effect && overridden || (parent_effect.f & 16384) !== 0) return d.v;
		return get(d);
	});
}
if (typeof HTMLElement === "function");
//#endregion
//#region node_modules/svelte/src/internal/disclose-version.js
if (typeof window !== "undefined") ((window.__svelte ??= {}).v ??= /* @__PURE__ */ new Set()).add("5");
//#endregion
//#region src/lib/surfaces.ts
function phase(feed) {
	return {
		loaded: feed.loaded ?? false,
		stale_since: feed.stale_since ?? null,
		error: feed.error ?? null
	};
}
var BANDS = [
	"attention",
	"happening",
	"steering",
	"project"
];
var BAND_LABELS = {
	attention: "What needs you",
	happening: "See what is happening",
	steering: "Start and steer work",
	project: "Set up a project"
};
var registry = /* @__PURE__ */ new Map();
function register(surface) {
	if (registry.has(surface.id)) throw new Error(`two surfaces claim the id "${surface.id}"`);
	registry.set(surface.id, surface);
}
function listed$1() {
	return surfaces().filter((s) => s.nav !== false);
}
function surfaces() {
	return [...registry.values()].sort((a, b) => BANDS.indexOf(a.band) - BANDS.indexOf(b.band) || a.order - b.order || a.id.localeCompare(b.id));
}
function landing() {
	return surfaces()[0];
}
//#endregion
//#region src/lib/text.ts
function clip(value, max) {
	const chars = [...value];
	return chars.length <= max ? value : chars.slice(0, Math.max(0, max - 1)).join("") + "…";
}
function ago$1(seconds) {
	if (!Number.isFinite(seconds)) return "";
	const s = Math.max(0, Math.floor(seconds));
	if (s < 60) return `${s}s`;
	if (s < 3600) return `${Math.floor(s / 60)}m`;
	if (s < 86400) return `${Math.floor(s / 3600)}h`;
	return `${Math.floor(s / 86400)}d`;
}
function plural(n, one, many) {
	return n === 1 ? one : many;
}
function listed(xs) {
	if (xs.length <= 1) return xs[0] ?? "";
	return `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`;
}
//#endregion
//#region src/lib/frame.ts
function hideWindow() {
	if (typeof window === "undefined" || typeof window.__devplane_hide !== "function") return false;
	window.__devplane_hide();
	return true;
}
//#endregion
//#region src/lib/keys.ts
var bindings = [];
var listeners = /* @__PURE__ */ new Map();
function bind(b) {
	if (!b.label.trim()) throw new Error(`${b.surface}: ${b.combo} (${b.action}) has no label`);
	const combo = normalise(b.combo);
	const same = bindings.find((x) => x.combo === combo && x.surface === b.surface);
	if (same) throw new Error(`${b.combo} is bound twice in ${b.surface}: "${same.label}" (${same.action}) and "${b.label}" (${b.action})`);
	const global = bindings.find((x) => x.combo === combo && x.surface === "global");
	if (global && b.surface !== "global") throw new Error(`${b.surface} rebinds ${b.combo}, which is global: "${global.label}" (${global.action}) against "${b.label}" (${b.action})`);
	const shadowed = b.surface === "global" ? bindings.find((x) => x.combo === combo) : void 0;
	if (shadowed) throw new Error(`global ${b.combo} ("${b.label}", ${b.action}) is already bound in ${shadowed.surface}: "${shadowed.label}" (${shadowed.action})`);
	bindings.push({
		...b,
		combo
	});
}
function all() {
	return bindings;
}
function help(surface) {
	return [...bindings.filter((b) => b.surface === "global"), ...bindings.filter((b) => b.surface === surface)];
}
function onAction(action, fn) {
	let set = listeners.get(action);
	if (!set) {
		set = /* @__PURE__ */ new Set();
		listeners.set(action, set);
	}
	set.add(fn);
	return () => {
		set?.delete(fn);
	};
}
function run(action, surface) {
	const set = listeners.get(action);
	if (!set || set.size === 0) return false;
	for (const fn of set) if (fn(surface) === true) return true;
	return false;
}
var pending = null;
var pendingAt = 0;
var CHORD_MS = 800;
function combo(e) {
	const parts = [];
	if (e.metaKey || e.ctrlKey) parts.push("Mod");
	if (e.altKey) parts.push("Alt");
	const key = (e.altKey ? fromCode(e.code) : null) ?? keyName(e.key);
	if (e.shiftKey && key.length > 1) parts.push("Shift");
	parts.push(key);
	return parts.join("+");
}
function keyName(key) {
	switch (key) {
		case "Escape": return "Esc";
		case "ArrowUp": return "Up";
		case "ArrowDown": return "Down";
		case "ArrowLeft": return "Left";
		case "ArrowRight": return "Right";
		case " ": return "Space";
		default: return key;
	}
}
var CODES = {
	BracketLeft: "[",
	BracketRight: "]",
	Minus: "-",
	Equal: "=",
	Comma: ",",
	Period: ".",
	Slash: "/",
	Semicolon: ";",
	Quote: "'",
	Backquote: "`",
	Backslash: "\\"
};
function fromCode(code) {
	if (!code) return null;
	const letter = /^Key([A-Z])$/.exec(code);
	if (letter) return letter[1].toLowerCase();
	const digit = /^Digit([0-9])$/.exec(code);
	if (digit) return digit[1];
	return CODES[code] ?? null;
}
function mac() {
	if (typeof navigator === "undefined") return false;
	const n = navigator;
	return /mac/i.test(n.userAgentData?.platform ?? n.userAgent ?? "");
}
function spell(c) {
	const onMac = mac();
	return c.split(" ").map((chord) => {
		const parts = chord.split("+");
		const key = parts.pop() ?? "";
		const mods = parts.map((m) => m === "Mod" ? onMac ? "⌘" : "Ctrl" : m === "Alt" ? onMac ? "⌥" : "Alt" : m === "Shift" ? onMac ? "⇧" : "Shift" : m);
		const k = parts.length && key.length === 1 ? key.toUpperCase() : key;
		return onMac ? mods.join("") + k : [...mods, k].join("+");
	}).join(" ");
}
function normalise(c) {
	return c.split(" ").map((part) => part.split("+").map((k) => k === "Escape" ? "Esc" : k === "Cmd" || k === "Ctrl" || k === "Meta" ? "Mod" : k).join("+")).join(" ");
}
var NATIVE_KEYS = /* @__PURE__ */ new Set([
	"Enter",
	"Space",
	"Up",
	"Down",
	"Left",
	"Right"
]);
function native(e, c) {
	if (!NATIVE_KEYS.has(c)) return false;
	const t = e.target;
	if (!t || typeof t.closest !== "function") return false;
	return !!t.closest("button, a[href], summary, [role=button], [role=link], [role=tab], [role=option], [role=menuitem], [role=separator], [role=checkbox], [role=radio], [role=switch]");
}
function typing(e) {
	const t = e.target;
	if (!t) return false;
	const tag = t.tagName;
	return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || t.isContentEditable;
}
var fromField = (c) => c === "Esc" || c.startsWith("Mod+") || c.startsWith("Alt+");
function dispatch(surface, e) {
	if (e.isComposing) return false;
	const c = combo(e);
	const scoped = (x) => x.surface === surface || x.surface === "global";
	if (typing(e)) {
		if (!fromField(c)) return false;
		pending = null;
		const hit = bindings.find((b) => scoped(b) && b.combo === c);
		if (hit && run(hit.action, surface)) {
			e.preventDefault();
			return true;
		}
		return false;
	}
	if (native(e, c)) return false;
	const now = Date.now();
	if (pending && now - pendingAt < CHORD_MS) {
		const chord = `${pending} ${c}`;
		pending = null;
		const hit = bindings.find((b) => scoped(b) && b.combo === chord);
		if (hit && run(hit.action, surface)) {
			e.preventDefault();
			return true;
		}
	}
	pending = null;
	const hit = bindings.find((b) => scoped(b) && b.combo === c);
	if (hit && run(hit.action, surface)) {
		e.preventDefault();
		return true;
	}
	if (bindings.some((b) => scoped(b) && b.combo.startsWith(`${c} `))) {
		pending = c;
		pendingAt = now;
		e.preventDefault();
		return true;
	}
	return false;
}
bind({
	surface: "global",
	combo: "Mod+k",
	action: "open-palette",
	label: "command palette"
});
bind({
	surface: "global",
	combo: "?",
	action: "help",
	label: "keys bound here"
});
bind({
	surface: "global",
	combo: "Esc",
	action: "leave",
	label: "leave: close the help, the palette or the new-change form"
});
function bindList(surface, open) {
	bind({
		surface,
		combo: "Down",
		action: "next",
		label: "next"
	});
	bind({
		surface,
		combo: "j",
		action: "next",
		label: "next"
	});
	bind({
		surface,
		combo: "Up",
		action: "prev",
		label: "previous"
	});
	bind({
		surface,
		combo: "k",
		action: "prev",
		label: "previous"
	});
	bind({
		surface,
		combo: "Enter",
		action: "open",
		label: open
	});
	bind({
		surface,
		combo: "g g",
		action: "first",
		label: "first"
	});
	bind({
		surface,
		combo: "G",
		action: "last",
		label: "last"
	});
}
//#endregion
//#region src/lib/api.ts
var KEY$2 = "vp_token";
function claimToken() {
	try {
		const url = new URL(location.href);
		const fromUrl = url.searchParams.get("token");
		if (fromUrl) {
			sessionStorage.setItem(KEY$2, fromUrl);
			url.searchParams.delete("token");
			history.replaceState({}, "", url);
			return fromUrl;
		}
		return sessionStorage.getItem(KEY$2) ?? "";
	} catch {
		return "";
	}
}
var Unauthorised = class extends Error {
	constructor() {
		super("unauthorised — run `devplane open` again");
	}
};
var READ_TIMEOUT_MS = 1e4;
var Unreachable = class extends Error {
	kind;
	constructor(kind) {
		super(kind === "timeout" ? "Devplane is not answering (slow or wedged)" : "Devplane is not running — start it with `devplane serve`");
		this.kind = kind;
	}
};
var Refused = class extends Error {
	status;
	body;
	constructor(said, status, body) {
		super(said);
		this.status = status;
		this.body = body;
	}
};
var writes = (opts) => !!opts.method && !/^(GET|HEAD)$/i.test(opts.method);
async function api(path, opts = {}) {
	const token = claimToken();
	const timeout = writes(opts) ? {} : { signal: AbortSignal.timeout(READ_TIMEOUT_MS) };
	let res;
	try {
		res = await fetch(path, {
			...opts,
			headers: {
				...opts.body ? { "content-type": "application/json" } : {},
				...opts.headers ?? {},
				Authorization: `Bearer ${token}`
			},
			...timeout
		});
	} catch (e) {
		throw new Unreachable(e instanceof DOMException && e.name === "TimeoutError" ? "timeout" : "refused");
	}
	if (res.status === 401) throw new Unauthorised();
	if (!res.ok) {
		const text = await res.text().catch(() => "");
		let body = null;
		try {
			body = text ? JSON.parse(text) : null;
		} catch {
			body = null;
		}
		const error = body?.error;
		throw new Refused((typeof error === "string" ? error : body === null ? text.trim().slice(0, 300) : "") || `${path} → ${res.status}`, res.status, body);
	}
	return await res.json();
}
//#endregion
//#region src/lib/resource.svelte.ts
function failure(e, tell = "devplane doctor") {
	if (e instanceof Unauthorised) return {
		says: "this tab has no token",
		tell: "devplane open"
	};
	if (e instanceof Unreachable) return {
		says: e.message,
		tell: e.kind === "refused" ? "devplane serve" : "devplane doctor"
	};
	return {
		says: e instanceof Error ? e.message : String(e),
		tell
	};
}
function same$1(a, b) {
	if (a === b) return true;
	try {
		return JSON.stringify(a) === JSON.stringify(b);
	} catch {
		return false;
	}
}
function resource(url, opts = {}) {
	let data = /* @__PURE__ */ state(null);
	let fail = /* @__PURE__ */ state(null);
	let at = /* @__PURE__ */ state(null);
	const key = /* @__PURE__ */ user_derived(url);
	let gen = 0;
	let seq = 0;
	let landed = 0;
	let pending = 0;
	async function read(want, mine) {
		const n = ++seq;
		pending += 1;
		try {
			const r = await api(want);
			if (mine !== gen || n < landed) return;
			landed = n;
			if (!same$1(get(data), r)) set(data, r);
			set(fail, null);
			set(at, Date.now(), true);
		} catch (e) {
			if (mine !== gen || n < landed) return;
			landed = n;
			set(fail, failure(e, opts.tell?.()));
		} finally {
			if (mine === gen) pending = Math.max(0, pending - 1);
		}
	}
	user_effect(() => {
		const want = get(key);
		const mine = untrack(() => {
			gen += 1;
			pending = 0;
			landed = seq;
			set(data, null);
			set(fail, null);
			set(at, null);
			return gen;
		});
		if (!want) return;
		read(want, mine);
		if (!opts.every) return;
		const t = setInterval(() => {
			if (!document.hidden && pending === 0) read(want, mine);
		}, opts.every);
		return () => clearInterval(t);
	});
	return {
		get phase() {
			return get(data) !== null ? get(fail) ? "stale" : "ok" : get(fail) ? "failed" : "loading";
		},
		get data() {
			return get(data);
		},
		get failure() {
			return get(fail);
		},
		get at() {
			return get(at);
		},
		async reload() {
			const want = untrack(() => get(key));
			if (want) await read(want, gen);
		}
	};
}
async function copyText(text) {
	try {
		if (!navigator.clipboard?.writeText) return false;
		await navigator.clipboard.writeText(text);
		return true;
	} catch {
		return false;
	}
}
//#endregion
//#region src/lib/write.svelte.ts
function writer() {
	let busy = /* @__PURE__ */ state("");
	let since = /* @__PURE__ */ state(0);
	let now = /* @__PURE__ */ state(0);
	let tick = null;
	return {
		get busy() {
			return get(busy);
		},
		get elapsed() {
			return get(busy) ? ago$1(Math.max(0, (get(now) - get(since)) / 1e3)) : "";
		},
		async run(what, work) {
			if (get(busy)) return null;
			set(busy, what, true);
			set(since, set(now, Date.now(), true), true);
			tick = setInterval(() => set(now, Date.now(), true), 1e3);
			try {
				return await work();
			} finally {
				if (tick !== null) clearInterval(tick);
				tick = null;
				set(busy, "");
			}
		}
	};
}
//#endregion
//#region src/lib/Failed.svelte
var root$50 = /* @__PURE__ */ from_html(`<p role="alert"><!> <span class="tell svelte-fezoay"><code class="svelte-fezoay"> </code> tells more.</span></p>`);
function Failed($$anchor, $$props) {
	push($$props, true);
	let at = prop($$props, "at", 3, null), stale = prop($$props, "stale", 3, false);
	var p = root$50();
	let classes;
	var node = child(p);
	var consequent = ($$anchor) => {
		var text$7 = text();
		template_effect(($0) => set_text(text$7, `Showing ${$$props.what ?? ""} as read ${$0 ?? ""} — the re-read failed: ${$$props.failure.says ?? ""}.`), [() => at() ? `${ago$1((Date.now() - at()) / 1e3)} ago` : "earlier"]);
		append($$anchor, text$7);
	};
	var alternate = ($$anchor) => {
		var text_1 = text();
		template_effect(() => set_text(text_1, `Could not read ${$$props.what ?? ""}: ${$$props.failure.says ?? ""}.`));
		append($$anchor, text_1);
	};
	if_block(node, ($$render) => {
		if (stale()) $$render(consequent);
		else $$render(alternate, -1);
	});
	var span = sibling(node, 2);
	var text_2 = only_child(child(span), true);
	next();
	reset(span);
	reset(p);
	template_effect(() => {
		classes = set_class(p, 1, "failed svelte-fezoay", null, classes, { stale: stale() });
		set_text(text_2, $$props.failure.tell);
	});
	append($$anchor, p);
	pop();
}
//#endregion
//#region src/lib/Commands.svelte
var root$49 = /* @__PURE__ */ from_html(`<span class="note svelte-1hip6z1" role="status">Copied.</span>`);
var root_1$48 = /* @__PURE__ */ from_html(`<span class="note svelte-1hip6z1" role="status">Nothing was copied — this page has no clipboard; select the text.</span>`);
var root_2$36 = /* @__PURE__ */ from_html(`<div class="commands svelte-1hip6z1"><pre class="svelte-1hip6z1"> </pre> <button type="button">Copy</button> <!> <!></div>`);
function Commands($$anchor, $$props) {
	push($$props, true);
	let copied = /* @__PURE__ */ state("");
	async function copy() {
		set(copied, await copyText($$props.commands.join("\n")) ? "yes" : "no", true);
	}
	var div = root_2$36();
	var pre = child(div);
	var text = only_child(pre, true);
	var button = sibling(pre, 2);
	var node = sibling(button, 2);
	var consequent = ($$anchor) => {
		append($$anchor, root$49());
	};
	if_block(node, ($$render) => {
		if (get(copied) === "yes") $$render(consequent);
	});
	var node_1 = sibling(node, 2);
	var consequent_1 = ($$anchor) => {
		append($$anchor, root_1$48());
	};
	if_block(node_1, ($$render) => {
		if (get(copied) === "no") $$render(consequent_1);
	});
	reset(div);
	template_effect(($0) => set_text(text, $0), [() => $$props.commands.join("\n")]);
	delegated("click", button, copy);
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/lib/State.svelte
var STATES = {
	working: {
		glyph: "●",
		word: "working",
		tone: "t-work"
	},
	"needs you": {
		glyph: "◆",
		word: "needs you",
		tone: "t-wait"
	},
	failed: {
		glyph: "✕",
		word: "failed",
		tone: "t-fail"
	},
	verified: {
		glyph: "✓",
		word: "verified",
		tone: "t-done"
	},
	idle: {
		glyph: "○",
		word: "idle",
		tone: "t-dim"
	},
	drafted: {
		glyph: "·",
		word: "drafted",
		tone: "t-dim"
	},
	isolated: {
		glyph: "⎇",
		word: "isolated",
		tone: "t-ink"
	},
	"in flight": {
		glyph: "▶",
		word: "in flight",
		tone: "t-work"
	},
	offered: {
		glyph: "↗",
		word: "offered",
		tone: "t-ink"
	},
	archived: {
		glyph: "▣",
		word: "archived",
		tone: "t-dim"
	},
	"no gates declared": {
		glyph: "∅",
		word: "no gates declared",
		tone: "t-wait"
	},
	stale: {
		glyph: "≠",
		word: "stale",
		tone: "t-wait"
	}
};
var LIFE = [
	"drafted",
	"isolated",
	"in flight",
	"verified",
	"offered",
	"archived"
];
var isArchived = (state) => state === STATES.archived.word;
var root$48 = /* @__PURE__ */ from_html(`<span class="sr-only"> </span>`);
var root_1$47 = /* @__PURE__ */ from_html(`<span><span class="glyph svelte-10029po" aria-hidden="true"> </span><!></span>`);
function State($$anchor, $$props) {
	let quiet = prop($$props, "quiet", 3, false);
	const row = /* @__PURE__ */ user_derived(() => STATES[$$props.state]);
	const said = /* @__PURE__ */ user_derived(() => $$props.word ?? get(row).word);
	var span = root_1$47();
	var span_1 = child(span);
	var text$6 = only_child(span_1, true);
	var node = sibling(span_1);
	var consequent = ($$anchor) => {};
	var consequent_1 = ($$anchor) => {
		var span_2 = root$48();
		var text_1 = only_child(span_2, true);
		template_effect(() => set_text(text_1, get(said)));
		append($$anchor, span_2);
	};
	var alternate = ($$anchor) => {
		var text_2 = text();
		template_effect(() => set_text(text_2, get(said)));
		append($$anchor, text_2);
	};
	if_block(node, ($$render) => {
		if (!get(said)) $$render(consequent);
		else if (quiet()) $$render(consequent_1, 1);
		else $$render(alternate, -1);
	});
	reset(span);
	template_effect(() => {
		set_class(span, 1, `state ${get(row).tone ?? ""}`, "svelte-10029po");
		set_text(text$6, get(row).glyph);
	});
	append($$anchor, span);
}
//#endregion
//#region src/lib/Qualifier.svelte
var root$47 = /* @__PURE__ */ from_html(`<span class="unseen svelte-1lbadw3"> </span>`);
var root_1$46 = /* @__PURE__ */ from_html(`<span><span class="sep svelte-1lbadw3" aria-hidden="true">·</span> <!></span>`);
function Qualifier($$anchor, $$props) {
	push($$props, true);
	let compact = prop($$props, "compact", 3, false);
	const shown = /* @__PURE__ */ user_derived(() => !!$$props.q && $$props.q.says !== "");
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_1 = ($$anchor) => {
		var span = root_1$46();
		let classes;
		var text = sibling(child(span));
		var node_1 = sibling(text);
		var consequent = ($$anchor) => {
			var span_1 = root$47();
			var text_1 = only_child(span_1);
			template_effect(() => set_text(text_1, `(${$$props.q.unseen ?? ""} unseen)`));
			append($$anchor, span_1);
		};
		if_block(node_1, ($$render) => {
			if ($$props.q.unseen > 0) $$render(consequent);
		});
		reset(span);
		template_effect(() => {
			classes = set_class(span, 1, "qualifier svelte-1lbadw3", null, classes, { compact: compact() });
			set_attribute(span, "title", $$props.q.unseen > 0 ? `${$$props.q.says} — ${$$props.q.unseen} not yet marked seen` : `${$$props.q.says} — every one marked seen`);
			set_text(text, ` ${$$props.q.says ?? ""}`);
		});
		append($$anchor, span);
	};
	if_block(node, ($$render) => {
		if (get(shown) && $$props.q) $$render(consequent_1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
//#region src/surfaces/review/marks.ts
function digest(text) {
	let h = 2166136261;
	for (let i = 0; i < text.length; i++) {
		h ^= text.charCodeAt(i);
		h = Math.imul(h, 16777619);
	}
	return (h >>> 0).toString(16).padStart(8, "0");
}
var prefix = (change) => `review:${change}:`;
function markKey(change, path, header, lines) {
	const body = lines.map(([kind, text]) => `${kind}${text}`).join("\n");
	return `${prefix(change)}${path}:${digest(`${header}\n${body}`)}`;
}
function readMark(key) {
	try {
		return localStorage.getItem(key) === "seen" ? "seen" : null;
	} catch {
		return null;
	}
}
function writeMark(key, mark) {
	try {
		localStorage.setItem(key, mark);
	} catch {}
}
function prune(change, live) {
	try {
		const gone = [];
		for (let i = 0; i < localStorage.length; i++) {
			const k = localStorage.key(i);
			if (k?.startsWith(prefix(change)) && !live.has(k)) gone.push(k);
		}
		gone.forEach((k) => localStorage.removeItem(k));
	} catch {}
}
function markedFor(change) {
	try {
		let n = 0;
		for (let i = 0; i < localStorage.length; i++) {
			const k = localStorage.key(i);
			if (k?.startsWith(prefix(change)) && readMark(k)) n++;
		}
		return n;
	} catch {
		return 0;
	}
}
//#endregion
//#region src/lib/href.ts
var SCHEMES = /* @__PURE__ */ new Set([
	"http:",
	"https:",
	"devplane:",
	"vscode:",
	"claude-cli:"
]);
function safeHref(url) {
	if (!url) return null;
	const s = url.trim();
	if (s.startsWith("#")) return s;
	const m = /^([a-z][a-z0-9+.-]*:)/i.exec(s);
	if (!m) return null;
	return SCHEMES.has(m[1].toLowerCase()) ? s : null;
}
//#endregion
//#region src/surfaces/inbox/Item.svelte
var root$46 = /* @__PURE__ */ from_html(`<span class="unseen svelte-kz8fai">new</span>`);
var root_1$45 = /* @__PURE__ */ from_html(`<span> </span>`);
var root_2$35 = /* @__PURE__ */ from_html(`<pre class="quoted svelte-kz8fai" aria-label="the report, quoted as it was filed"> </pre>`);
var root_3$32 = /* @__PURE__ */ from_html(`<p class="d svelte-kz8fai"> </p>`);
var root_4$29 = /* @__PURE__ */ from_html(`<p class="facts svelte-kz8fai"><!> <span title="hunks marked in this browser's review"> </span> <!></p>`);
var root_5$25 = /* @__PURE__ */ from_html(`<button> </button>`);
var root_6$24 = /* @__PURE__ */ from_html(`<div class="reply svelte-kz8fai"><input type="text" placeholder="your answer" class="svelte-kz8fai"/> <button>reply</button></div>`);
var root_7$21 = /* @__PURE__ */ from_html(`<fieldset class="q svelte-kz8fai"><legend class="svelte-kz8fai"> </legend> <div class="opts svelte-kz8fai"></div> <!></fieldset>`);
var root_8$21 = /* @__PURE__ */ from_html(`<div class="reply svelte-kz8fai"><button> </button></div>`);
var root_9$18 = /* @__PURE__ */ from_html(`<!> <!>`, 1);
var root_10$18 = /* @__PURE__ */ from_html(`<button class="choice svelte-kz8fai"> </button>`);
var root_11$14 = /* @__PURE__ */ from_html(`<span class="dead svelte-kz8fai"> </span>`);
var root_12$12 = /* @__PURE__ */ from_html(`<div class="opts svelte-kz8fai"></div>`);
var root_13$12 = /* @__PURE__ */ from_html(`<p class="offer svelte-kz8fai"><span class="dim svelte-kz8fai">never asked again:</span> <code class="svelte-kz8fai"> </code> <button>copy</button> <span class="dim svelte-kz8fai"> </span></p>`);
var root_14$12 = /* @__PURE__ */ from_html(`<p class="elsewhere svelte-kz8fai"> </p>`);
var root_15$9 = /* @__PURE__ */ from_html(`<button>allow</button>`);
var root_16$6 = /* @__PURE__ */ from_html(`<button>deny</button>`);
var root_17$4 = /* @__PURE__ */ from_html(`<button>retry</button>`);
var root_18$2 = /* @__PURE__ */ from_html(`<button>resume</button>`);
var root_19$2 = /* @__PURE__ */ from_html(`<a class="act primary svelte-kz8fai">review</a>`);
var root_20 = /* @__PURE__ */ from_html(`<button>tell the run</button>`);
var root_21 = /* @__PURE__ */ from_html(`<button>accept the drift</button>`);
var root_22 = /* @__PURE__ */ from_html(`<a class="act svelte-kz8fai">open</a>`);
var root_23 = /* @__PURE__ */ from_html(`<a class="act svelte-kz8fai" target="_blank" rel="noreferrer noopener"> </a>`);
var root_24 = /* @__PURE__ */ from_html(`<a class="act svelte-kz8fai">open an agent with this typed</a>`);
var root_25 = /* @__PURE__ */ from_html(`<button>start a change from this</button>`);
var root_26 = /* @__PURE__ */ from_html(`<button>reject</button>`);
var root_27 = /* @__PURE__ */ from_html(`<button>defer</button>`);
var root_28 = /* @__PURE__ */ from_html(`<input class="why svelte-kz8fai" type="text" placeholder="why — the filer is told" aria-label="why, for the project that filed it"/> <!> <!>`, 1);
var root_29 = /* @__PURE__ */ from_html(`<button>open on GitHub</button>`);
var root_30 = /* @__PURE__ */ from_html(`<button>discard the draft</button>`);
var root_31 = /* @__PURE__ */ from_html(`<button>snooze 1h</button>`);
var root_32 = /* @__PURE__ */ from_html(`<code class="cmd svelte-kz8fai"> </code>`);
var root_33 = /* @__PURE__ */ from_html(`<li><span class="lvl svelte-kz8fai"><!></span> <div class="body svelte-kz8fai"><div class="t svelte-kz8fai"><b class="svelte-kz8fai"> </b> <span class="kind svelte-kz8fai"> </span> <!> <span class="meta svelte-kz8fai"><!> <!></span></div> <!> <!> <!> <!> <!> <div class="acts svelte-kz8fai"><!> <!> <!> <!> <!> <!> <!> <!> <!> <!> <!> <!> <!> <!> <!> <!></div></div></li>`);
function Item($$anchor, $$props) {
	push($$props, true);
	const DETAIL_CHARS = 400;
	let current = prop($$props, "current", 3, false), age = prop($$props, "age", 3, "");
	const acts = /* @__PURE__ */ user_derived(() => $$props.item.actions ?? []);
	const markedHunks = /* @__PURE__ */ user_derived(() => $$props.item.facts && $$props.item.change_id ? Math.min(markedFor($$props.item.change_id), $$props.item.facts.hunks) : 0);
	const has = (a) => get(acts).includes(a);
	const url = /* @__PURE__ */ user_derived(() => safeHref($$props.item.url));
	const launch = /* @__PURE__ */ user_derived(() => safeHref($$props.item.launch));
	const loud = /* @__PURE__ */ user_derived(() => $$props.item.level === "critical" || $$props.item.level === "high");
	let typed = /* @__PURE__ */ state("");
	let custom = proxy({});
	let reason = /* @__PURE__ */ state("");
	function answerReport(action) {
		const why = get(reason).trim();
		if (!why) {
			$$props.say("say why first — the project that filed it is told the reason");
			return;
		}
		$$props.act($$props.item, action, why);
	}
	function reply(field) {
		const words = (field ? custom[field] ?? "" : get(typed)).trim();
		if (!words) {
			$$props.say("type your answer first — an empty reply is not an answer");
			return;
		}
		if (field) pick(field, { custom: words });
		else $$props.answer($$props.item, { custom: words });
	}
	const form = /* @__PURE__ */ user_derived(() => ($$props.item.form ?? []).length > 0 ? $$props.item.form ?? [] : null);
	let picked = /* @__PURE__ */ state(proxy({}));
	function pick(field, what) {
		if ((get(form) ?? []).length <= 1) {
			$$props.answer($$props.item, what.custom !== void 0 ? {
				custom: what.custom,
				field
			} : {
				option: what.option,
				field
			});
			return;
		}
		set(picked, {
			...get(picked),
			[field]: what
		}, true);
	}
	const outstanding = /* @__PURE__ */ user_derived(() => (get(form) ?? []).filter((q) => !get(picked)[q.field]).length);
	function send() {
		$$props.answer($$props.item, { answers: (get(form) ?? []).map((q) => ({
			field: q.field,
			...get(picked)[q.field]
		})) });
	}
	var li = root_33();
	let classes;
	var span = child(li);
	var node = child(span);
	var consequent = ($$anchor) => {
		State($$anchor, {
			state: "needs you",
			get word() {
				return $$props.item.level;
			}
		});
	};
	var alternate = ($$anchor) => {
		State($$anchor, {
			state: "idle",
			get word() {
				return $$props.item.level;
			},
			quiet: true
		});
	};
	if_block(node, ($$render) => {
		if (get(loud)) $$render(consequent);
		else $$render(alternate, -1);
	});
	reset(span);
	var div = sibling(span, 2);
	var div_1 = child(div);
	var b = child(div_1);
	var text = only_child(b, true);
	var span_1 = sibling(b, 2);
	var text_1 = only_child(span_1, true);
	var node_1 = sibling(span_1, 2);
	var consequent_1 = ($$anchor) => {
		append($$anchor, root$46());
	};
	if_block(node_1, ($$render) => {
		if ($$props.item.new_to_you) $$render(consequent_1);
	});
	var span_3 = sibling(node_1, 2);
	var node_2 = child(span_3);
	var consequent_2 = ($$anchor) => {
		var span_4 = root_1$45();
		var text_2 = only_child(span_4, true);
		template_effect(() => set_text(text_2, $$props.item.project_name));
		append($$anchor, span_4);
	};
	if_block(node_2, ($$render) => {
		if ($$props.item.project_name) $$render(consequent_2);
	});
	var node_3 = sibling(node_2, 2);
	var consequent_3 = ($$anchor) => {
		var span_5 = root_1$45();
		var text_3 = only_child(span_5, true);
		template_effect(() => {
			set_attribute(span_5, "title", $$props.item.since);
			set_text(text_3, age());
		});
		append($$anchor, span_5);
	};
	if_block(node_3, ($$render) => {
		if (age()) $$render(consequent_3);
	});
	reset(span_3);
	reset(div_1);
	var node_4 = sibling(div_1, 2);
	var consequent_4 = ($$anchor) => {
		var pre = root_2$35();
		var text_4 = only_child(pre, true);
		template_effect(() => set_text(text_4, $$props.item.detail));
		append($$anchor, pre);
	};
	var consequent_5 = ($$anchor) => {
		var p = root_3$32();
		var text_5 = only_child(p, true);
		template_effect(($0) => {
			set_attribute(p, "title", $$props.item.detail.length > DETAIL_CHARS ? $$props.item.detail : void 0);
			set_text(text_5, $0);
		}, [() => clip($$props.item.detail, DETAIL_CHARS)]);
		append($$anchor, p);
	};
	if_block(node_4, ($$render) => {
		if ($$props.item.detail && $$props.item.report) $$render(consequent_4);
		else if ($$props.item.detail) $$render(consequent_5, 1);
	});
	var node_5 = sibling(node_4, 2);
	var consequent_6 = ($$anchor) => {
		var p_1 = root_4$29();
		var node_6 = child(p_1);
		Qualifier(node_6, { get q() {
			return $$props.item.facts.qualifier;
		} });
		var span_6 = sibling(node_6, 2);
		var text_6 = only_child(span_6);
		each(sibling(span_6, 2), 16, () => $$props.item.facts.says, (s) => s, ($$anchor, s) => {
			var span_7 = root_1$45();
			var text_7 = only_child(span_7);
			template_effect(() => set_text(text_7, `· ${s ?? ""}`));
			append($$anchor, span_7);
		});
		reset(p_1);
		template_effect(() => set_text(text_6, `${get(markedHunks) ?? ""} of ${$$props.item.facts.hunks ?? ""} ${$$props.item.facts.hunks === 1 ? "hunk" : "hunks"} marked`));
		append($$anchor, p_1);
	};
	if_block(node_5, ($$render) => {
		if ($$props.item.facts) $$render(consequent_6);
	});
	var node_8 = sibling(node_5, 2);
	var consequent_9 = ($$anchor) => {
		var fragment_2 = root_9$18();
		var node_9 = first_child(fragment_2);
		each(node_9, 17, () => get(form), (q) => q.field, ($$anchor, q) => {
			var fieldset = root_7$21();
			var legend = child(fieldset);
			var text_8 = only_child(legend, true);
			var div_2 = sibling(legend, 2);
			each(div_2, 21, () => get(q).options, index, ($$anchor, o) => {
				var button = root_5$25();
				let classes_1;
				var text_9 = only_child(button, true);
				template_effect(() => {
					classes_1 = set_class(button, 1, "choice svelte-kz8fai", null, classes_1, { picked: get(picked)[get(q).field]?.option === get(o).value });
					set_attribute(button, "title", get(o).detail ?? void 0);
					set_text(text_9, get(o).label);
				});
				delegated("click", button, () => pick(get(q).field, { option: get(o).value }));
				append($$anchor, button);
			});
			reset(div_2);
			var node_10 = sibling(div_2, 2);
			var consequent_7 = ($$anchor) => {
				var div_3 = root_6$24();
				var input = child(div_3);
				remove_input_defaults(input);
				var button_1 = sibling(input, 2);
				reset(div_3);
				template_effect(() => set_attribute(input, "aria-label", `your answer to: ${get(q).title ?? ""}`));
				bind_value(input, () => custom[get(q).field], ($$value) => custom[get(q).field] = $$value);
				delegated("click", button_1, () => reply(get(q).field));
				append($$anchor, div_3);
			};
			if_block(node_10, ($$render) => {
				if (get(q).custom_field) $$render(consequent_7);
			});
			reset(fieldset);
			template_effect(() => set_text(text_8, get(q).title));
			append($$anchor, fieldset);
		});
		var node_11 = sibling(node_9, 2);
		var consequent_8 = ($$anchor) => {
			var div_4 = root_8$21();
			var button_2 = child(div_4);
			var text_10 = only_child(button_2, true);
			reset(div_4);
			template_effect(() => {
				button_2.disabled = get(outstanding) > 0;
				set_text(text_10, get(outstanding) > 0 ? `${get(outstanding)} of ${get(form).length} still to answer` : `send ${get(form).length} answers`);
			});
			delegated("click", button_2, send);
			append($$anchor, div_4);
		};
		if_block(node_11, ($$render) => {
			if (get(form).length > 1) $$render(consequent_8);
		});
		append($$anchor, fragment_2);
	};
	var d = /* @__PURE__ */ user_derived(() => get(form) && (has("choose") || has("reply")));
	var alternate_2 = ($$anchor) => {
		var fragment_3 = root_9$18();
		var node_12 = first_child(fragment_3);
		var consequent_11 = ($$anchor) => {
			var div_5 = root_12$12();
			each(div_5, 21, () => $$props.item.options ?? [], index, ($$anchor, o) => {
				var fragment_4 = comment();
				var node_13 = first_child(fragment_4);
				var consequent_10 = ($$anchor) => {
					var button_3 = root_10$18();
					var text_11 = only_child(button_3, true);
					template_effect(() => set_text(text_11, get(o).label));
					delegated("click", button_3, () => $$props.answer($$props.item, { option: get(o).id }));
					append($$anchor, button_3);
				};
				var alternate_1 = ($$anchor) => {
					var span_8 = root_11$14();
					var text_12 = only_child(span_8, true);
					template_effect(() => set_text(text_12, get(o).label));
					append($$anchor, span_8);
				};
				if_block(node_13, ($$render) => {
					if (get(o).id) $$render(consequent_10);
					else $$render(alternate_1, -1);
				});
				append($$anchor, fragment_4);
			});
			reset(div_5);
			append($$anchor, div_5);
		};
		var d_1 = /* @__PURE__ */ user_derived(() => ($$props.item.options ?? []).length > 0 && has("choose"));
		if_block(node_12, ($$render) => {
			if (get(d_1)) $$render(consequent_11);
		});
		var node_14 = sibling(node_12, 2);
		var consequent_12 = ($$anchor) => {
			var div_6 = root_6$24();
			var input_1 = child(div_6);
			remove_input_defaults(input_1);
			var button_4 = sibling(input_1, 2);
			reset(div_6);
			template_effect(() => set_attribute(input_1, "aria-label", `your answer to: ${$$props.item.title ?? ""}`));
			bind_value(input_1, () => get(typed), ($$value) => set(typed, $$value));
			delegated("click", button_4, () => reply());
			append($$anchor, div_6);
		};
		var d_2 = /* @__PURE__ */ user_derived(() => has("reply"));
		if_block(node_14, ($$render) => {
			if (get(d_2)) $$render(consequent_12);
		});
		append($$anchor, fragment_3);
	};
	if_block(node_8, ($$render) => {
		if (get(d)) $$render(consequent_9);
		else $$render(alternate_2, -1);
	});
	var node_15 = sibling(node_8, 2);
	var consequent_13 = ($$anchor) => {
		var p_2 = root_13$12();
		var code = sibling(child(p_2), 2);
		var text_13 = only_child(code, true);
		var button_5 = sibling(code, 2);
		var text_14 = only_child(sibling(button_5, 2));
		reset(p_2);
		template_effect(() => {
			set_text(text_13, $$props.item.offer.rule);
			set_text(text_14, `paste into ${$$props.item.offer.file ?? ""} ${$$props.item.offer.section ?? ""} · covers ${$$props.item.offer.covers ?? ""}${$$props.item.offer.more ? "+" : ""} like it`);
		});
		delegated("click", button_5, () => $$props.copyRule($$props.item.offer.rule));
		append($$anchor, p_2);
	};
	var consequent_14 = ($$anchor) => {
		var p_3 = root_14$12();
		var text_15 = only_child(p_3, true);
		template_effect(() => set_text(text_15, $$props.item.no_offer.sentence));
		append($$anchor, p_3);
	};
	if_block(node_15, ($$render) => {
		if ($$props.item.offer) $$render(consequent_13);
		else if ($$props.item.no_offer) $$render(consequent_14, 1);
	});
	var node_16 = sibling(node_15, 2);
	var consequent_15 = ($$anchor) => {
		var p_4 = root_14$12();
		var text_16 = only_child(p_4, true);
		template_effect(() => set_text(text_16, $$props.item.answer_in));
		append($$anchor, p_4);
	};
	if_block(node_16, ($$render) => {
		if ($$props.item.answer_in) $$render(consequent_15);
	});
	var div_7 = sibling(node_16, 2);
	var node_17 = child(div_7);
	var consequent_16 = ($$anchor) => {
		var button_6 = root_15$9();
		delegated("click", button_6, () => $$props.answer($$props.item, { decision: "allow" }));
		append($$anchor, button_6);
	};
	var d_3 = /* @__PURE__ */ user_derived(() => has("allow"));
	if_block(node_17, ($$render) => {
		if (get(d_3)) $$render(consequent_16);
	});
	var node_18 = sibling(node_17, 2);
	var consequent_17 = ($$anchor) => {
		var button_7 = root_16$6();
		delegated("click", button_7, () => $$props.answer($$props.item, { decision: "deny" }));
		append($$anchor, button_7);
	};
	var d_4 = /* @__PURE__ */ user_derived(() => has("deny"));
	if_block(node_18, ($$render) => {
		if (get(d_4)) $$render(consequent_17);
	});
	var node_19 = sibling(node_18, 2);
	var consequent_18 = ($$anchor) => {
		var button_8 = root_17$4();
		delegated("click", button_8, () => $$props.act($$props.item, "retry"));
		append($$anchor, button_8);
	};
	var d_5 = /* @__PURE__ */ user_derived(() => has("retry"));
	if_block(node_19, ($$render) => {
		if (get(d_5)) $$render(consequent_18);
	});
	var node_20 = sibling(node_19, 2);
	var consequent_19 = ($$anchor) => {
		var button_9 = root_18$2();
		delegated("click", button_9, () => $$props.act($$props.item, "resume"));
		append($$anchor, button_9);
	};
	var d_6 = /* @__PURE__ */ user_derived(() => has("resume"));
	if_block(node_20, ($$render) => {
		if (get(d_6)) $$render(consequent_19);
	});
	var node_21 = sibling(node_20, 2);
	var consequent_20 = ($$anchor) => {
		var a_1 = root_19$2();
		template_effect(($0) => set_attribute(a_1, "href", $0), [() => `#review/${encodeURIComponent($$props.item.change_id)}`]);
		append($$anchor, a_1);
	};
	var d_7 = /* @__PURE__ */ user_derived(() => has("review") && $$props.item.change_id);
	if_block(node_21, ($$render) => {
		if (get(d_7)) $$render(consequent_20);
	});
	var node_22 = sibling(node_21, 2);
	var consequent_21 = ($$anchor) => {
		var button_10 = root_20();
		delegated("click", button_10, () => $$props.act($$props.item, "tell_run"));
		append($$anchor, button_10);
	};
	var d_8 = /* @__PURE__ */ user_derived(() => has("tell_run"));
	if_block(node_22, ($$render) => {
		if (get(d_8)) $$render(consequent_21);
	});
	var node_23 = sibling(node_22, 2);
	var consequent_22 = ($$anchor) => {
		var button_11 = root_21();
		delegated("click", button_11, () => $$props.act($$props.item, "accept_drift"));
		append($$anchor, button_11);
	};
	var d_9 = /* @__PURE__ */ user_derived(() => has("accept_drift"));
	if_block(node_23, ($$render) => {
		if (get(d_9)) $$render(consequent_22);
	});
	var node_24 = sibling(node_23, 2);
	var consequent_23 = ($$anchor) => {
		var a_2 = root_22();
		template_effect(($0) => set_attribute(a_2, "href", $0), [() => $$props.item.change_id ? `#change/${encodeURIComponent($$props.item.change_id)}` : `#why/${encodeURIComponent($$props.item.run_id ?? "")}`]);
		append($$anchor, a_2);
	};
	var d_10 = /* @__PURE__ */ user_derived(() => has("open") && ($$props.item.run_id || $$props.item.change_id));
	if_block(node_24, ($$render) => {
		if (get(d_10)) $$render(consequent_23);
	});
	var node_25 = sibling(node_24, 2);
	var consequent_24 = ($$anchor) => {
		var a_3 = root_23();
		var text_17 = only_child(a_3, true);
		template_effect(($0) => {
			set_attribute(a_3, "href", get(url));
			set_text(text_17, $0);
		}, [() => has("open_pr") ? "open pull request" : "open issue"]);
		append($$anchor, a_3);
	};
	var d_11 = /* @__PURE__ */ user_derived(() => (has("open_pr") || has("open_issue")) && get(url));
	if_block(node_25, ($$render) => {
		if (get(d_11)) $$render(consequent_24);
	});
	var node_26 = sibling(node_25, 2);
	var consequent_25 = ($$anchor) => {
		var a_4 = root_24();
		template_effect(() => set_attribute(a_4, "href", get(launch)));
		append($$anchor, a_4);
	};
	if_block(node_26, ($$render) => {
		if (get(launch)) $$render(consequent_25);
	});
	var node_27 = sibling(node_26, 2);
	var consequent_26 = ($$anchor) => {
		var button_12 = root_25();
		delegated("click", button_12, () => $$props.act($$props.item, "start_from_report"));
		append($$anchor, button_12);
	};
	var d_12 = /* @__PURE__ */ user_derived(() => has("start_from_report"));
	if_block(node_27, ($$render) => {
		if (get(d_12)) $$render(consequent_26);
	});
	var node_28 = sibling(node_27, 2);
	var consequent_29 = ($$anchor) => {
		var fragment_5 = root_28();
		var input_2 = first_child(fragment_5);
		remove_input_defaults(input_2);
		var node_29 = sibling(input_2, 2);
		var consequent_27 = ($$anchor) => {
			var button_13 = root_26();
			delegated("click", button_13, () => answerReport("reject_report"));
			append($$anchor, button_13);
		};
		var d_13 = /* @__PURE__ */ user_derived(() => has("reject_report"));
		if_block(node_29, ($$render) => {
			if (get(d_13)) $$render(consequent_27);
		});
		var node_30 = sibling(node_29, 2);
		var consequent_28 = ($$anchor) => {
			var button_14 = root_27();
			delegated("click", button_14, () => answerReport("defer_report"));
			append($$anchor, button_14);
		};
		var d_14 = /* @__PURE__ */ user_derived(() => has("defer_report"));
		if_block(node_30, ($$render) => {
			if (get(d_14)) $$render(consequent_28);
		});
		bind_value(input_2, () => get(reason), ($$value) => set(reason, $$value));
		append($$anchor, fragment_5);
	};
	var d_15 = /* @__PURE__ */ user_derived(() => has("reject_report") || has("defer_report"));
	if_block(node_28, ($$render) => {
		if (get(d_15)) $$render(consequent_29);
	});
	var node_31 = sibling(node_28, 2);
	var consequent_30 = ($$anchor) => {
		var button_15 = root_29();
		delegated("click", button_15, () => $$props.act($$props.item, "open_draft"));
		append($$anchor, button_15);
	};
	var d_16 = /* @__PURE__ */ user_derived(() => has("open_draft"));
	if_block(node_31, ($$render) => {
		if (get(d_16)) $$render(consequent_30);
	});
	var node_32 = sibling(node_31, 2);
	var consequent_31 = ($$anchor) => {
		var button_16 = root_30();
		delegated("click", button_16, () => $$props.act($$props.item, "discard_draft"));
		append($$anchor, button_16);
	};
	var d_17 = /* @__PURE__ */ user_derived(() => has("discard_draft"));
	if_block(node_32, ($$render) => {
		if (get(d_17)) $$render(consequent_31);
	});
	var node_33 = sibling(node_32, 2);
	var consequent_32 = ($$anchor) => {
		var button_17 = root_31();
		delegated("click", button_17, () => $$props.snooze($$props.item));
		append($$anchor, button_17);
	};
	var d_18 = /* @__PURE__ */ user_derived(() => has("snooze"));
	if_block(node_33, ($$render) => {
		if (get(d_18)) $$render(consequent_32);
	});
	var node_34 = sibling(node_33, 2);
	var consequent_33 = ($$anchor) => {
		var code_1 = root_32();
		var text_18 = only_child(code_1);
		template_effect(() => set_text(text_18, `devplane attach ${$$props.item.run_id ?? ""}`));
		append($$anchor, code_1);
	};
	var d_19 = /* @__PURE__ */ user_derived(() => has("attach") && $$props.item.run_id);
	if_block(node_34, ($$render) => {
		if (get(d_19)) $$render(consequent_33);
	});
	reset(div_7);
	reset(div);
	reset(li);
	template_effect(($0) => {
		classes = set_class(li, 1, "item svelte-kz8fai", null, classes, {
			loud: get(loud),
			current: current()
		});
		set_attribute(li, "id", $$props.item.ask ? `row-${$$props.item.ask}` : void 0);
		set_attribute(li, "aria-current", current() ? "true" : void 0);
		set_text(text, $$props.item.title);
		set_text(text_1, $0);
	}, [() => $$props.item.kind.replace(/_/g, " ")]);
	append($$anchor, li);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/inbox/actions.ts
function outcome(r, ours) {
	const commands = [r?.push, r?.create].filter((c) => !!c);
	if (commands.length) return {
		said: r?.says ?? "Nothing was pushed — run these yourself:",
		undo: null,
		commands
	};
	return {
		said: r?.says ?? ours,
		undo: null
	};
}
var ROUTES = {
	retry: {
		of: "change",
		route: "/api/changes/{id}/retry",
		says: "retrying"
	},
	resume: {
		of: "change",
		route: "/api/changes/{id}/resume",
		says: "resumed"
	},
	tell_run: {
		of: "change",
		route: "/api/changes/{id}/drift/tell",
		says: "told — the run was handed the files that changed, and the decision is recorded as yours",
		body: (item) => ({ run: item.run_id })
	},
	accept_drift: {
		of: "change",
		route: "/api/changes/{id}/drift/accept",
		says: "accepted — the change now works to what the run saw, and the decision is recorded as yours",
		body: (item) => ({ run: item.run_id })
	}
};
var REPORT_ROUTES = {
	start_from_report: {
		route: "/api/reports/{id}/start",
		says: "started a change from it — the report is attached, and is answered as fixed when that change is offered or finished"
	},
	reject_report: {
		route: "/api/reports/{id}/resolve",
		says: "rejected — the project that filed it is told why",
		body: (reason) => ({
			as: "rejected",
			reason
		})
	},
	defer_report: {
		route: "/api/reports/{id}/resolve",
		says: "deferred — the project that filed it is told why",
		body: (reason) => ({
			as: "deferred",
			reason
		})
	},
	open_draft: {
		route: "/api/reports/{id}/open",
		says: "opened on GitHub under your sign-in, in your name"
	},
	discard_draft: {
		route: "/api/reports/{id}/resolve",
		says: "discarded — nothing was sent",
		body: () => ({ as: "discarded" })
	}
};
var failed = (e) => e instanceof Error ? e.message : String(e);
async function actOnReport(item, action, reason) {
	const r = REPORT_ROUTES[action];
	if (!r || !item.report) return {
		said: `${action} cannot be done from here`,
		undo: null
	};
	try {
		return outcome(await api(r.route.replace("{id}", encodeURIComponent(item.report)), {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(r.body ? r.body(reason) : {})
		}), r.says);
	} catch (e) {
		return {
			said: `${action.replace(/_/g, " ")} did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function act$1(item, action, reason = "") {
	if (REPORT_ROUTES[action]) return actOnReport(item, action, reason);
	const r = ROUTES[action];
	const id = r?.of === "change" ? item.change_id : item.run_id;
	if (!r || !id) return {
		said: `${action} cannot be done from here`,
		undo: null
	};
	try {
		return outcome(await api(r.route.replace("{id}", encodeURIComponent(id)), {
			method: "POST",
			...r.body ? { body: JSON.stringify(r.body(item)) } : {}
		}), r.says);
	} catch (e) {
		return {
			said: `${action} did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function answer(item, what) {
	const ask = item.ask ?? item.request_id;
	if (!ask) return {
		said: "this one cannot be answered from here",
		undo: null
	};
	try {
		await api(`/api/asks/${encodeURIComponent(ask)}/answer`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				...what,
				from: "board"
			})
		});
		return {
			said: "answered — on the record and on its way to the agent. That cannot be taken back.",
			undo: null
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function snooze(item) {
	const where = item.change_id ? `/api/changes/${encodeURIComponent(item.change_id)}/snooze` : item.run_id ? `/api/runs/${encodeURIComponent(item.run_id)}/snooze` : item.project_id ? `/api/projects/${encodeURIComponent(item.project_id)}/snooze` : null;
	if (!where) return {
		said: "there is nothing to snooze this against",
		undo: null
	};
	try {
		await api(`${where}?minutes=60`, { method: "POST" });
		return {
			said: "hidden for an hour",
			undo: {
				says: "put it back",
				where: `${where}?minutes=0`
			}
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function takeBack(undo) {
	try {
		await api(undo.where, { method: "POST" });
		return {
			said: "back in the list",
			undo: null
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function copyRule(rule) {
	return await copyText(rule) ? {
		said: `copied: ${rule}`,
		undo: null
	} : {
		said: "nothing was copied: this page has no clipboard — select the rule above",
		undo: null
	};
}
//#endregion
//#region src/surfaces/answer/Answer.svelte
var root$45 = /* @__PURE__ */ from_html(`<p class="dim svelte-1umb941">reading…</p>`);
var root_1$44 = /* @__PURE__ */ from_html(`<p class="dim more svelte-1umb941"> </p>`);
var root_2$34 = /* @__PURE__ */ from_html(`<!> <fieldset class="bare svelte-1umb941"><ul role="list" class="svelte-1umb941"><!></ul></fieldset> <!>`, 1);
var root_3$31 = /* @__PURE__ */ from_html(`<p class="dim svelte-1umb941">Nothing needs you.</p>`);
var root_4$28 = /* @__PURE__ */ from_html(`<section class="answer svelte-1umb941" aria-labelledby="answer-head"><h2 id="answer-head" class="svelte-1umb941">The one thing that needs you</h2> <p class="said svelte-1umb941" role="status" aria-live="polite"> </p> <!> <!> <p class="hint dim svelte-1umb941">Esc hides this</p></section>`);
function Answer($$anchor, $$props) {
	push($$props, true);
	let items = prop($$props, "items", 3, null);
	const read = resource(() => "/api/inbox?needs_you=true", {
		every: 2e3,
		tell: () => "devplane inbox --needs-you"
	});
	let said = /* @__PURE__ */ state("");
	let commands = /* @__PURE__ */ state(proxy([]));
	const shown = /* @__PURE__ */ user_derived(() => read.data?.items ?? items() ?? []);
	const first = /* @__PURE__ */ user_derived(() => get(shown)[0] ?? null);
	const failed = /* @__PURE__ */ user_derived(() => read.phase === "failed" && items() === null);
	const loaded = /* @__PURE__ */ user_derived(() => read.data !== null || items() !== null);
	user_effect(() => onAction("leave", () => {
		hideWindow();
		return true;
	}));
	const pending = writer();
	async function report(work) {
		const r = await pending.run("action", work);
		if (!r) return;
		set(said, r.said, true);
		set(commands, r.commands ?? [], true);
		await read.reload();
	}
	const answer$2 = (item, what) => report(() => answer(item, what));
	const act = (item, action, reason) => report(() => act$1(item, action, reason));
	const snooze$2 = (item) => report(() => snooze(item));
	async function copyRule$2(rule) {
		set(said, (await copyRule(rule)).said, true);
	}
	let now = /* @__PURE__ */ state(proxy(Date.now()));
	user_effect(() => {
		const id = setInterval(() => set(now, Date.now(), true), 3e4);
		return () => clearInterval(id);
	});
	var section = root_4$28();
	var p = sibling(child(section), 2);
	var text = only_child(p, true);
	var node = sibling(p, 2);
	var consequent = ($$anchor) => {
		Commands($$anchor, { get commands() {
			return get(commands);
		} });
	};
	if_block(node, ($$render) => {
		if (get(commands).length) $$render(consequent);
	});
	var node_1 = sibling(node, 2);
	var consequent_1 = ($$anchor) => {
		Failed($$anchor, {
			what: "what needs you",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_2 = ($$anchor) => {
		append($$anchor, root$45());
	};
	var consequent_5 = ($$anchor) => {
		var fragment_2 = root_2$34();
		var node_2 = first_child(fragment_2);
		var consequent_3 = ($$anchor) => {
			Failed($$anchor, {
				what: "what needs you",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				stale: true
			});
		};
		if_block(node_2, ($$render) => {
			if (read.phase === "stale" && read.failure) $$render(consequent_3);
		});
		var fieldset = sibling(node_2, 2);
		var ul = child(fieldset);
		key$1(child(ul), () => get(first).id, ($$anchor) => {
			{
				let $0 = /* @__PURE__ */ user_derived(() => get(first).since ? ago$1(Math.max(0, (get(now) - Date.parse(get(first).since)) / 1e3)) : "");
				Item($$anchor, {
					get item() {
						return get(first);
					},
					current: true,
					get age() {
						return get($0);
					},
					answer: answer$2,
					act,
					snooze: snooze$2,
					copyRule: copyRule$2,
					say: (s) => set(said, s, true)
				});
			}
		});
		reset(ul);
		reset(fieldset);
		var node_4 = sibling(fieldset, 2);
		var consequent_4 = ($$anchor) => {
			var p_2 = root_1$44();
			var text_1 = only_child(p_2);
			template_effect(() => set_text(text_1, `${get(shown).length - 1} more after this one`));
			append($$anchor, p_2);
		};
		if_block(node_4, ($$render) => {
			if (get(shown).length > 1) $$render(consequent_4);
		});
		template_effect(() => fieldset.disabled = !!pending.busy);
		append($$anchor, fragment_2);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_5 = comment();
		var node_5 = first_child(fragment_5);
		var consequent_6 = ($$anchor) => {
			Failed($$anchor, {
				what: "what needs you",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				stale: true
			});
		};
		var alternate = ($$anchor) => {
			append($$anchor, root_3$31());
		};
		if_block(node_5, ($$render) => {
			if (read.phase === "stale" && read.failure) $$render(consequent_6);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment_5);
	};
	if_block(node_1, ($$render) => {
		if (get(failed) && read.failure) $$render(consequent_1);
		else if (!get(loaded)) $$render(consequent_2, 1);
		else if (get(first)) $$render(consequent_5, 2);
		else $$render(alternate_1, -1);
	});
	next(2);
	reset(section);
	template_effect(() => set_text(text, get(said)));
	append($$anchor, section);
	pop();
}
//#endregion
//#region src/surfaces/answer/index.ts
register({
	id: "answer",
	title: "Answer",
	heading: "The one thing that needs you",
	band: "attention",
	order: 9,
	nav: false,
	bare: true,
	reads: ["/api/inbox"],
	select: () => ({}),
	component: Answer
});
//#endregion
//#region src/lib/route.ts
function go(hash) {
	history.replaceState({}, "", hash || "#");
	dispatchEvent(new HashChangeEvent("hashchange"));
}
//#endregion
//#region src/lib/ui/Icon.svelte
var PATHS = {
	inbox: "M3 13h5l1.5 3h5L16 13h5M5.5 5h13L21 13v6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-6z",
	change: "M6 3v12M18 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 9a9 9 0 0 1-9 9",
	sessions: "M3 12h4l3-8 4 16 3-8h4",
	spec: "M14 3H6a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8zM14 3v5h5M9 13h6M9 17h6M9 9h2",
	ledger: "M12 3v18M5 7h14M5 7l-2.5 6a3 3 0 0 0 5 0zM19 7l-2.5 6a3 3 0 0 0 5 0zM8 21h8",
	report: "M5 21V4M5 4h11l-2 4 2 4H5",
	forge: "M6 3v12M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 15V9a3 3 0 0 0-3-3h-4M13 3l-2 3 2 3",
	settings: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-2.9 1.2V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-2.9-1.2l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.7 1.7 0 0 0 3.9 14H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.2-2.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.7 1.7 0 0 0 10 3.1V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 2.9 1.2l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0 1.2 2.9H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
	search: "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM21 21l-4.3-4.3",
	plus: "M12 5v14M5 12h14",
	play: "M7 4l13 8-13 8z",
	stop: "M6 6h12v12H6z",
	check: "M4 12.5l5 5L20 6.5",
	x: "M6 6l12 12M18 6L6 18",
	alert: "M12 9v4M12 17h.01M10.3 3.9L2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
	question: "M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3M12 17h.01M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z",
	clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3 2",
	right: "M9 6l6 6-6 6",
	down: "M6 9l6 6 6-6",
	left: "M15 6l-6 6 6 6",
	dot: "M12 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2z",
	sidebar: "M4 4h16v16H4zM9 4v16",
	panel: "M4 4h16v16H4zM4 15h16",
	split: "M4 4h16v16H4zM12 4v16",
	unified: "M4 4h16v16H4zM4 12h16",
	filter: "M3 5h18l-7 8v6l-4 2v-8z",
	refresh: "M21 12a9 9 0 1 1-2.6-6.4M21 4v5h-5",
	external: "M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
	terminal: "M4 17l6-5-6-5M12 19h8",
	file: "M14 3H6a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8zM14 3v5h5",
	folder: "M3 6a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z",
	person: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21a8 8 0 0 1 16 0",
	agent: "M12 8V4M8 4h8M6 8h12a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2zM9 13v1M15 13v1",
	shield: "M12 21s8-4 8-10V5l-8-3-8 3v6c0 6 8 10 8 10z",
	rule: "M4 6h16M4 12h10M4 18h7",
	gate: "M4 21V8l8-5 8 5v13M9 21v-6h6v6",
	timeline: "M3 12h18M7 8v8M12 5v14M17 9v6",
	keyboard: "M3 6h18v12H3zM7 10h.01M11 10h.01M15 10h.01M7 14h10",
	sun: "M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4",
	moon: "M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8z",
	more: "M5 12h.01M12 12h.01M19 12h.01",
	pin: "M12 17v5M9 3h6l-1 6 3 3H7l3-3z",
	eye: "M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
	skip: "M5 4l10 8-10 8zM19 5v14"
};
var root$44 = /* @__PURE__ */ from_svg(`<svg class="icon svelte-jf3o41" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><path></path></svg>`);
function Icon($$anchor, $$props) {
	let size = prop($$props, "size", 3, 16), label = prop($$props, "label", 3, ""), stroke = prop($$props, "stroke", 3, 1.75);
	const d = /* @__PURE__ */ user_derived(() => PATHS[$$props.name] ?? PATHS.dot);
	var svg = root$44();
	var path = only_child(svg);
	template_effect(() => {
		set_attribute(svg, "width", size());
		set_attribute(svg, "height", size());
		set_attribute(svg, "stroke-width", stroke());
		set_attribute(svg, "role", label() ? "img" : void 0);
		set_attribute(svg, "aria-label", label() || void 0);
		set_attribute(svg, "aria-hidden", label() ? void 0 : "true");
		set_attribute(path, "d", get(d));
	});
	append($$anchor, svg);
}
//#endregion
//#region src/lib/ui/Grid.svelte
function nextSort(by, c) {
	if (!c.sort) return by;
	if (by?.key !== c.key) return {
		key: c.key,
		dir: 1
	};
	if (by.dir === 1) return {
		key: c.key,
		dir: -1
	};
	return null;
}
function sorted(rows, columns, by) {
	if (!by) return rows;
	const col = columns.find((c) => c.key === by.key);
	if (!col?.sort) return rows;
	const f = col.sort;
	return [...rows].sort((a, b) => {
		const x = f(a);
		const y = f(b);
		if (x == null && y == null) return 0;
		if (x == null) return 1;
		if (y == null) return -1;
		return (x < y ? -1 : x > y ? 1 : 0) * by.dir;
	});
}
var root$43 = /* @__PURE__ */ from_html(`<div role="columnheader"><span> </span> <!> <span class="grip svelte-jwljd6" role="presentation"></span></div>`);
var root_1$43 = /* @__PURE__ */ from_html(`<span> </span>`);
var root_2$33 = /* @__PURE__ */ from_html(`<button class="group svelte-jwljd6"><!> <!> <span class="n svelte-jwljd6"> </span></button>`);
var root_3$30 = /* @__PURE__ */ from_html(`<div role="gridcell"><!></div>`);
var root_4$27 = /* @__PURE__ */ from_html(`<div class="row svelte-jwljd6" role="row" tabindex="-1"></div>`);
var root_5$24 = /* @__PURE__ */ from_html(`<!> <!>`, 1);
var root_6$23 = /* @__PURE__ */ from_html(`<div role="grid" tabindex="0"><div class="head svelte-jwljd6" role="row"></div> <div class="body svelte-jwljd6"><!></div></div>`);
function Grid($$anchor, $$props) {
	push($$props, true);
	let group = prop($$props, "group", 3, void 0), groupLabel = prop($$props, "groupLabel", 3, void 0), selected = prop($$props, "selected", 15, null), open = prop($$props, "open", 3, void 0), empty = prop($$props, "empty", 3, void 0), dense = prop($$props, "dense", 3, false), label = prop($$props, "label", 3, "rows");
	const wkey = () => `vp-grid:${$$props.id}`;
	function load() {
		try {
			return JSON.parse(localStorage.getItem(wkey()) ?? "{}");
		} catch {
			return {};
		}
	}
	let widths = /* @__PURE__ */ state(proxy(load()));
	const template = /* @__PURE__ */ user_derived(() => $$props.columns.map((c, i) => {
		const w = get(widths)[c.key] ?? c.width;
		return w ? `${w}px` : i === $$props.columns.length - 1 ? "minmax(8rem, 1fr)" : "auto";
	}).join(" "));
	let resizing = null;
	function grab(e, c) {
		e.stopPropagation();
		const th = e.currentTarget.parentElement;
		resizing = {
			key: c.key,
			x: e.clientX,
			from: th.getBoundingClientRect().width
		};
		e.currentTarget.setPointerCapture(e.pointerId);
	}
	function drag(e) {
		if (!resizing) return;
		set(widths, {
			...get(widths),
			[resizing.key]: Math.max(48, resizing.from + e.clientX - resizing.x)
		}, true);
	}
	function drop() {
		if (!resizing) return;
		resizing = null;
		try {
			localStorage.setItem(wkey(), JSON.stringify(get(widths)));
		} catch {}
	}
	let by = /* @__PURE__ */ state(null);
	function toggle(c) {
		set(by, nextSort(get(by), c), true);
	}
	const ordered = /* @__PURE__ */ user_derived(() => sorted($$props.rows, $$props.columns, get(by)));
	let folded = /* @__PURE__ */ state(proxy({}));
	const groups = /* @__PURE__ */ user_derived(() => {
		if (!group()) return [{
			name: "",
			rows: get(ordered)
		}];
		const out = [];
		const at = /* @__PURE__ */ new Map();
		for (const r of get(ordered)) {
			const g = group()(r);
			if (!at.has(g)) {
				at.set(g, out.length);
				out.push({
					name: g,
					rows: []
				});
			}
			out[at.get(g)].rows.push(r);
		}
		return out;
	});
	const visible = /* @__PURE__ */ user_derived(() => get(groups).flatMap((g) => get(folded)[g.name] ? [] : g.rows));
	let body = /* @__PURE__ */ state(void 0);
	function keydown(e) {
		if (get(visible).length === 0) return;
		const i = get(visible).findIndex((r) => $$props.key(r) === selected());
		let next = i;
		if (e.key === "ArrowDown" || e.key === "j") next = Math.min(get(visible).length - 1, i + 1);
		else if (e.key === "ArrowUp" || e.key === "k") next = Math.max(0, i === -1 ? 0 : i - 1);
		else if (e.key === "Home") next = 0;
		else if (e.key === "End") next = get(visible).length - 1;
		else if (e.key === "Enter" && i !== -1) {
			e.preventDefault();
			open()?.(get(visible)[i]);
			return;
		} else return;
		e.preventDefault();
		selected($$props.key(get(visible)[next]));
		queueMicrotask(() => get(body)?.querySelector(`[data-key="${CSS.escape(selected() ?? "")}"]`)?.scrollIntoView({ block: "nearest" }));
	}
	var div = root_6$23();
	let classes;
	var div_1 = child(div);
	each(div_1, 21, () => $$props.columns, (c) => c.key, ($$anchor, c) => {
		var div_2 = root$43();
		let classes_1;
		var span = child(div_2);
		var text = only_child(span, true);
		var node = sibling(span, 2);
		var consequent = ($$anchor) => {
			{
				let $0 = /* @__PURE__ */ user_derived(() => get(by).dir === 1 ? "down" : "right");
				Icon($$anchor, {
					get name() {
						return get($0);
					},
					size: 12
				});
			}
		};
		if_block(node, ($$render) => {
			if (get(by)?.key === get(c).key) $$render(consequent);
		});
		var span_1 = sibling(node, 2);
		reset(div_2);
		template_effect(($0) => {
			classes_1 = set_class(div_2, 1, "th svelte-jwljd6", null, classes_1, {
				end: get(c).align === "end",
				sortable: !!get(c).sort
			});
			set_attribute(div_2, "tabindex", get(c).sort ? 0 : -1);
			set_attribute(div_2, "aria-sort", get(by)?.key === get(c).key ? get(by).dir === 1 ? "ascending" : "descending" : "none");
			set_attribute(div_2, "title", $0);
			set_text(text, get(c).label);
		}, [() => get(c).sort ? `sort by ${get(c).label.toLowerCase()} (Enter or Space)` : void 0]);
		delegated("click", div_2, () => toggle(get(c)));
		delegated("keydown", div_2, (e) => {
			if (get(c).sort && (e.key === "Enter" || e.key === " ")) {
				e.preventDefault();
				e.stopPropagation();
				toggle(get(c));
			}
		});
		delegated("pointerdown", span_1, (e) => grab(e, get(c)));
		delegated("pointermove", span_1, drag);
		delegated("pointerup", span_1, drop);
		event("pointercancel", span_1, drop);
		append($$anchor, div_2);
	});
	reset(div_1);
	var div_3 = sibling(div_1, 2);
	var node_1 = child(div_3);
	var consequent_2 = ($$anchor) => {
		var fragment_1 = comment();
		var node_2 = first_child(fragment_1);
		var consequent_1 = ($$anchor) => {
			var fragment_2 = comment();
			snippet(first_child(fragment_2), empty);
			append($$anchor, fragment_2);
		};
		if_block(node_2, ($$render) => {
			if (empty()) $$render(consequent_1);
		});
		append($$anchor, fragment_1);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_3 = comment();
		each(first_child(fragment_3), 17, () => get(groups), (g) => g.name, ($$anchor, g) => {
			var fragment_4 = root_5$24();
			var node_5 = first_child(fragment_4);
			var consequent_4 = ($$anchor) => {
				var button = root_2$33();
				var node_6 = child(button);
				{
					let $0 = /* @__PURE__ */ user_derived(() => get(folded)[get(g).name] ? "right" : "down");
					Icon(node_6, {
						get name() {
							return get($0);
						},
						size: 12
					});
				}
				var node_7 = sibling(node_6, 2);
				var consequent_3 = ($$anchor) => {
					var fragment_5 = comment();
					snippet(first_child(fragment_5), groupLabel, () => get(g).name, () => get(g).rows.length);
					append($$anchor, fragment_5);
				};
				var alternate = ($$anchor) => {
					var span_2 = root_1$43();
					var text_1 = only_child(span_2, true);
					template_effect(() => set_text(text_1, get(g).name));
					append($$anchor, span_2);
				};
				if_block(node_7, ($$render) => {
					if (groupLabel()) $$render(consequent_3);
					else $$render(alternate, -1);
				});
				var text_2 = only_child(sibling(node_7, 2), true);
				reset(button);
				template_effect(() => set_text(text_2, get(g).rows.length));
				delegated("click", button, () => set(folded, {
					...get(folded),
					[get(g).name]: !get(folded)[get(g).name]
				}, true));
				append($$anchor, button);
			};
			if_block(node_5, ($$render) => {
				if (group()) $$render(consequent_4);
			});
			var node_9 = sibling(node_5, 2);
			var consequent_5 = ($$anchor) => {
				var fragment_6 = comment();
				each(first_child(fragment_6), 17, () => get(g).rows, (r) => $$props.key(r), ($$anchor, r) => {
					var div_4 = root_4$27();
					each(div_4, 21, () => $$props.columns, (c) => c.key, ($$anchor, c) => {
						var div_5 = root_3$30();
						let classes_2;
						snippet(child(div_5), () => $$props.cell, () => get(r), () => get(c));
						reset(div_5);
						template_effect(() => classes_2 = set_class(div_5, 1, "td svelte-jwljd6", null, classes_2, {
							end: get(c).align === "end",
							mono: get(c).mono
						}));
						append($$anchor, div_5);
					});
					reset(div_4);
					template_effect(($0, $1) => {
						set_attribute(div_4, "data-key", $0);
						set_attribute(div_4, "aria-selected", $1);
					}, [() => $$props.key(get(r)), () => selected() === $$props.key(get(r))]);
					delegated("click", div_4, () => selected($$props.key(get(r))));
					delegated("dblclick", div_4, () => open()?.(get(r)));
					delegated("keydown", div_4, () => {});
					append($$anchor, div_4);
				});
				append($$anchor, fragment_6);
			};
			if_block(node_9, ($$render) => {
				if (!get(folded)[get(g).name]) $$render(consequent_5);
			});
			append($$anchor, fragment_4);
		});
		append($$anchor, fragment_3);
	};
	if_block(node_1, ($$render) => {
		if ($$props.rows.length === 0) $$render(consequent_2);
		else $$render(alternate_1, -1);
	});
	reset(div_3);
	bind_this(div_3, ($$value) => set(body, $$value), () => get(body));
	reset(div);
	template_effect(() => {
		classes = set_class(div, 1, "grid svelte-jwljd6", null, classes, { dense: dense() });
		set_attribute(div, "aria-label", label());
		set_attribute(div, "aria-rowcount", $$props.rows.length);
		set_style(div, `--cols: ${get(template) ?? ""}`);
	});
	delegated("keydown", div, keydown);
	append($$anchor, div);
	pop();
}
delegate([
	"keydown",
	"click",
	"pointerdown",
	"pointermove",
	"pointerup",
	"dblclick"
]);
//#endregion
//#region src/lib/ui/Pill.svelte
var TONES = [
	["fail", /fail|stopped|broken|denied|deny|refused|error|lost|abandon/i],
	["wait", /need|wait|ask|question|permission|held|stale|drift|blocked|review/i],
	["done", /^(verified|passed)$/i],
	["work", /work|flight|running|isolated|driving|live|active|in progress/i]
];
function tone(word) {
	if (!word) return "none";
	for (const [t, re] of TONES) if (re.test(word)) return t;
	return "none";
}
var root$42 = /* @__PURE__ */ from_html(`<span class="dot svelte-ueztnz" aria-hidden="true"></span>`);
var root_1$42 = /* @__PURE__ */ from_html(`<span><!> </span>`);
function Pill($$anchor, $$props) {
	let as = prop($$props, "as", 3, void 0), dot = prop($$props, "dot", 3, true), title = prop($$props, "title", 3, void 0);
	const t = /* @__PURE__ */ user_derived(() => as() ?? tone($$props.word));
	var span = root_1$42();
	var node = child(span);
	var consequent = ($$anchor) => {
		append($$anchor, root$42());
	};
	if_block(node, ($$render) => {
		if (dot()) $$render(consequent);
	});
	var text = sibling(node, 1, true);
	reset(span);
	template_effect(() => {
		set_class(span, 1, `pill ${get(t) ?? ""}`, "svelte-ueztnz");
		set_attribute(span, "title", title());
		set_text(text, $$props.word);
	});
	append($$anchor, span);
}
//#endregion
//#region src/lib/ui/Empty.svelte
var root$41 = /* @__PURE__ */ from_html(`<p class="body svelte-x83d2f"> </p>`);
var root_1$41 = /* @__PURE__ */ from_html(`<p class="limit svelte-x83d2f"><!> </p>`);
var root_2$32 = /* @__PURE__ */ from_html(`<div class="action svelte-x83d2f"><!></div>`);
var root_3$29 = /* @__PURE__ */ from_html(`<div class="empty svelte-x83d2f"><span class="glyph svelte-x83d2f"><!></span> <p class="title svelte-x83d2f"> </p> <!> <!> <!></div>`);
function Empty($$anchor, $$props) {
	let icon = prop($$props, "icon", 3, "dot"), body = prop($$props, "body", 3, ""), limit = prop($$props, "limit", 3, "");
	var div = root_3$29();
	var span = child(div);
	Icon(child(span), {
		get name() {
			return icon();
		},
		size: 22
	});
	reset(span);
	var p = sibling(span, 2);
	var text = only_child(p, true);
	var node_1 = sibling(p, 2);
	var consequent = ($$anchor) => {
		var p_1 = root$41();
		var text_1 = only_child(p_1, true);
		template_effect(() => set_text(text_1, body()));
		append($$anchor, p_1);
	};
	if_block(node_1, ($$render) => {
		if (body()) $$render(consequent);
	});
	var node_2 = sibling(node_1, 2);
	var consequent_1 = ($$anchor) => {
		var p_2 = root_1$41();
		var node_3 = child(p_2);
		Icon(node_3, {
			name: "eye",
			size: 13
		});
		var text_2 = sibling(node_3);
		reset(p_2);
		template_effect(() => set_text(text_2, ` ${limit() ?? ""}`));
		append($$anchor, p_2);
	};
	if_block(node_2, ($$render) => {
		if (limit()) $$render(consequent_1);
	});
	var node_4 = sibling(node_2, 2);
	var consequent_2 = ($$anchor) => {
		var div_1 = root_2$32();
		snippet(child(div_1), () => $$props.action);
		reset(div_1);
		append($$anchor, div_1);
	};
	if_block(node_4, ($$render) => {
		if ($$props.action) $$render(consequent_2);
	});
	reset(div);
	template_effect(() => set_text(text, $$props.title));
	append($$anchor, div);
}
//#endregion
//#region src/lib/ui/Props.svelte
var root$40 = /* @__PURE__ */ from_html(`<dt class="svelte-1gth30k"> </dt> <dd> </dd>`, 1);
var root_1$40 = /* @__PURE__ */ from_html(`<dl class="props svelte-1gth30k"><!> <!></dl>`);
function Props($$anchor, $$props) {
	var dl = root_1$40();
	var node = child(dl);
	each(node, 17, () => $$props.rows, (r) => r.label, ($$anchor, r) => {
		var fragment = root$40();
		var dt = first_child(fragment);
		var text = only_child(dt, true);
		var dd = sibling(dt, 2);
		let classes;
		var text_1 = only_child(dd, true);
		template_effect(() => {
			set_text(text, get(r).label);
			classes = set_class(dd, 1, clsx(get(r).tone ?? ""), "svelte-1gth30k", classes, {
				mono: get(r).mono,
				missing: get(r).value == null || get(r).value === ""
			});
			set_text(text_1, get(r).value == null || get(r).value === "" ? get(r).missing ?? "—" : get(r).value);
		});
		append($$anchor, fragment);
	});
	var node_1 = sibling(node, 2);
	var consequent = ($$anchor) => {
		var fragment_1 = comment();
		snippet(first_child(fragment_1), () => $$props.extra);
		append($$anchor, fragment_1);
	};
	if_block(node_1, ($$render) => {
		if ($$props.extra) $$render(consequent);
	});
	reset(dl);
	append($$anchor, dl);
}
//#endregion
//#region src/lib/Skeleton.svelte
var root$39 = /* @__PURE__ */ from_html(`<div class="row svelte-1xuy3ds"><span class="bar svelte-1xuy3ds"></span></div>`);
var root_1$39 = /* @__PURE__ */ from_html(`<div class="skeleton svelte-1xuy3ds" aria-busy="true" aria-label="reading"></div>`);
function Skeleton($$anchor, $$props) {
	let rows = prop($$props, "rows", 3, 3);
	var div = root_1$39();
	each(div, 21, () => ({ length: rows() }), index, ($$anchor, _, i) => {
		var div_1 = root$39();
		set_style(child(div_1), "", {}, { width: `${40 + i * 23 % 40}%` });
		reset(div_1);
		append($$anchor, div_1);
	});
	reset(div);
	append($$anchor, div);
}
//#endregion
//#region src/surfaces/board/Board.svelte
var root$38 = /* @__PURE__ */ from_html(`<button> <span class="svelte-13ck13b"> </span></button>`);
var root_1$38 = /* @__PURE__ */ from_html(`<div class="chips svelte-13ck13b" role="group" aria-label="by state"><button>all <span class="svelte-13ck13b"> </span></button> <!></div>`);
var root_2$31 = /* @__PURE__ */ from_html(`<span class="cost svelte-13ck13b" title="reported by the vendors' own telemetry"> </span>`);
var root_3$28 = /* @__PURE__ */ from_html(`<p class="quiet svelte-13ck13b">The last read had no session in it, and Devplane has not answered since.</p>`);
var root_4$26 = /* @__PURE__ */ from_html(`<span class="sess svelte-13ck13b"><!> <b class="svelte-13ck13b"> </b> <span class="dim svelte-13ck13b"> </span></span>`);
var root_5$23 = /* @__PURE__ */ from_html(`<span class="dim svelte-13ck13b"> </span>`);
var root_6$22 = /* @__PURE__ */ from_html(`<!><span class="sr-only">context nearly full</span>`, 1);
var root_7$20 = /* @__PURE__ */ from_html(`<span><!> </span>`);
var root_8$20 = /* @__PURE__ */ from_html(`<span class="dim svelte-13ck13b" title="not reported">—</span>`);
var root_9$17 = /* @__PURE__ */ from_html(`<span class="dim svelte-13ck13b">—</span>`);
var root_10$17 = /* @__PURE__ */ from_html(`<p class="quiet svelte-13ck13b"> </p>`);
var root_11$13 = /* @__PURE__ */ from_html(`<p class="said svelte-13ck13b"> </p>`);
var root_12$11 = /* @__PURE__ */ from_html(`<aside class="drawer svelte-13ck13b" aria-label="the session"><header class="svelte-13ck13b"><b> </b> <!> <button class="x svelte-13ck13b" aria-label="close"><!></button></header> <!> <div class="acts svelte-13ck13b"><button class="svelte-13ck13b"><!> Copy attach command</button></div> <!></aside>`);
var root_13$11 = /* @__PURE__ */ from_html(`<div class="frame svelte-13ck13b"><!> <!></div>`);
var root_14$11 = /* @__PURE__ */ from_html(`<div class="board svelte-13ck13b"><header class="head svelte-13ck13b"><h1 class="svelte-13ck13b">Sessions</h1> <!> <span class="gap svelte-13ck13b"></span> <label class="group svelte-13ck13b">Group <select aria-label="group by" class="svelte-13ck13b"><option>by project</option><option>by state</option><option>none</option></select></label> <!></header> <!></div>`);
function Board($$anchor, $$props) {
	push($$props, true);
	let runs = prop($$props, "runs", 19, () => []), summary = prop($$props, "summary", 3, null), thresholds = prop($$props, "thresholds", 3, null), watching = prop($$props, "watching", 3, null), loaded = prop($$props, "loaded", 3, false), error = prop($$props, "error", 3, null), focus = prop($$props, "focus", 3, "");
	const BUCKETS = [
		["working", "working"],
		["waiting", "needs_you"],
		["idle", "idle"],
		["failed", "failed"],
		["dormant", "dormant"]
	];
	const IN_PLAY_SECONDS = 21600;
	const bucket = (r) => {
		const s = r.state ?? "";
		const w = r.waiting_for ?? "";
		const waiting = s === "waiting";
		if (!(waiting && w !== "idle" || s === "working" || s === "starting" || (r.idle_seconds ?? 0) < IN_PLAY_SECONDS && (!!r.reporting || s === "failed" || s === "lost"))) return "dormant";
		if (s === "working" || s === "starting") return "working";
		if (waiting) return "waiting";
		if (s === "failed" || s === "lost") return "failed";
		return "idle";
	};
	let only = /* @__PURE__ */ state(null);
	let groupBy = /* @__PURE__ */ state("project");
	let selected = /* @__PURE__ */ state(null);
	user_effect(() => {
		if (focus()) set(selected, focus());
	});
	const counts = /* @__PURE__ */ user_derived(() => loaded() && summary() ? BUCKETS.map(([b, field]) => [b, summary()[field] ?? 0]) : null);
	const shown = /* @__PURE__ */ user_derived(() => get(only) ? runs().filter((r) => bucket(r) === get(only)) : runs());
	const pick = /* @__PURE__ */ user_derived(() => runs().find((r) => r.id === get(selected)) ?? null);
	const high = /* @__PURE__ */ user_derived(() => thresholds()?.context_high_percent ?? null);
	const crowded = (r) => get(high) != null && r.context_percent != null && r.context_percent >= get(high);
	const blind = /* @__PURE__ */ user_derived(() => [watching()?.driven_only?.length ? `${watching().driven_only.join(", ")} appear only when Devplane starts them.` : "", watching()?.unproved?.length ? `${watching().unproved.join(", ")} ${watching().unproved.length === 1 ? "is" : "are"} read, but has not been proved against a live session.` : ""].filter(Boolean).join(" "));
	function when(at) {
		if (!at) return "—";
		const s = Math.max(0, Math.round((Date.now() - Date.parse(at)) / 1e3));
		return s < 60 ? `${s}s` : s < 3600 ? `${Math.round(s / 60)}m` : s < 86400 ? `${Math.round(s / 3600)}h` : `${Math.round(s / 86400)}d`;
	}
	const columns = [
		{
			key: "state",
			label: "State",
			width: 130,
			sort: (r) => bucket(r)
		},
		{
			key: "session",
			label: "Session",
			width: 220,
			sort: (r) => r.name ?? r.agent ?? r.id
		},
		{
			key: "project",
			label: "Project",
			width: 120,
			sort: (r) => r.project_name ?? ""
		},
		{
			key: "doing",
			label: "Doing",
			width: 320
		},
		{
			key: "context",
			label: "Context",
			width: 90,
			align: "end",
			sort: (r) => r.context_percent ?? -1,
			mono: true
		},
		{
			key: "cost",
			label: "Cost",
			width: 80,
			align: "end",
			sort: (r) => r.cost_usd ?? -1,
			mono: true
		},
		{
			key: "tools",
			label: "Tools",
			width: 70,
			align: "end",
			sort: (r) => r.tool_calls ?? 0,
			mono: true
		},
		{
			key: "last",
			label: "Last",
			align: "end",
			sort: (r) => r.last_event_at ?? "",
			mono: true
		}
	];
	let said = /* @__PURE__ */ state("");
	async function copy(t) {
		set(said, await copyText(t) ? `Copied: ${t}` : `Nothing was copied — this page has no clipboard: ${t}`, true);
	}
	var div = root_14$11();
	var header = child(div);
	var node = sibling(child(header), 2);
	var consequent = ($$anchor) => {
		var div_1 = root_1$38();
		var button = child(div_1);
		let classes;
		var text = only_child(sibling(child(button)), true);
		reset(button);
		each(sibling(button, 2), 17, () => get(counts), ([b, n]) => b, ($$anchor, $$item) => {
			var $$array = /* @__PURE__ */ user_derived(() => to_array(get($$item), 2));
			let b = () => get($$array)[0];
			let n = () => get($$array)[1];
			var button_1 = root$38();
			let classes_1;
			var text_1 = child(button_1);
			var text_2 = only_child(sibling(text_1), true);
			reset(button_1);
			template_effect(() => {
				set_attribute(button_1, "aria-pressed", get(only) === b());
				classes_1 = set_class(button_1, 1, clsx(b()), "svelte-13ck13b", classes_1, { on: get(only) === b() });
				button_1.disabled = n() === 0;
				set_text(text_1, `${b() ?? ""} `);
				set_text(text_2, n());
			});
			delegated("click", button_1, () => set(only, get(only) === b() ? null : b(), true));
			append($$anchor, button_1);
		});
		reset(div_1);
		template_effect(() => {
			set_attribute(button, "aria-pressed", get(only) === null);
			classes = set_class(button, 1, "svelte-13ck13b", null, classes, { on: get(only) === null });
			set_text(text, summary()?.runs ?? runs().length);
		});
		delegated("click", button, () => set(only, null));
		append($$anchor, div_1);
	};
	if_block(node, ($$render) => {
		if (get(counts)) $$render(consequent);
	});
	var label = sibling(node, 4);
	var select = sibling(child(label));
	var option = child(select);
	option.value = option.__value = "project";
	var option_1 = sibling(option);
	option_1.value = option_1.__value = "state";
	var option_2 = sibling(option_1);
	option_2.value = option_2.__value = "none";
	reset(select);
	init_select(select);
	reset(label);
	var node_2 = sibling(label, 2);
	var consequent_1 = ($$anchor) => {
		var span_2 = root_2$31();
		var text_3 = only_child(span_2);
		template_effect(($0) => set_text(text_3, `$${$0 ?? ""} reported today`), [() => summary().cost_usd.toFixed(2)]);
		append($$anchor, span_2);
	};
	if_block(node_2, ($$render) => {
		if (summary()?.cost_usd) $$render(consequent_1);
	});
	reset(header);
	var node_3 = sibling(header, 2);
	var consequent_2 = ($$anchor) => {
		Skeleton($$anchor, {});
	};
	var consequent_3 = ($$anchor) => {
		append($$anchor, root_3$28());
	};
	var consequent_4 = ($$anchor) => {
		Empty($$anchor, {
			icon: "sessions",
			title: "No session is reporting",
			body: "Sessions you start yourself appear here as soon as they report — Claude Code with no configuration, others once connected.",
			get limit() {
				return get(blind);
			}
		});
	};
	var alternate_3 = ($$anchor) => {
		var div_2 = root_13$11();
		var node_4 = child(div_2);
		{
			const cell = ($$anchor, r = noop, c = noop) => {
				var fragment_2 = comment();
				var node_5 = first_child(fragment_2);
				var consequent_5 = ($$anchor) => {
					{
						let $0 = /* @__PURE__ */ user_derived(() => r().state ?? "unknown");
						Pill($$anchor, { get word() {
							return get($0);
						} });
					}
				};
				var consequent_6 = ($$anchor) => {
					var span_3 = root_4$26();
					var node_6 = child(span_3);
					{
						let $0 = /* @__PURE__ */ user_derived(() => r().mode === "driven" ? "agent" : "eye");
						Icon(node_6, {
							get name() {
								return get($0);
							},
							size: 13
						});
					}
					var b_1 = sibling(node_6, 2);
					var text_4 = only_child(b_1, true);
					var text_5 = only_child(sibling(b_1, 2), true);
					reset(span_3);
					template_effect(() => {
						set_text(text_4, r().name ?? r().agent ?? "session");
						set_text(text_5, r().model ?? "");
					});
					append($$anchor, span_3);
				};
				var consequent_7 = ($$anchor) => {
					var text_6 = text();
					template_effect(() => set_text(text_6, r().project_name ?? "—"));
					append($$anchor, text_6);
				};
				var consequent_8 = ($$anchor) => {
					var span_5 = root_5$23();
					var text_7 = only_child(span_5, true);
					template_effect(() => set_text(text_7, r().summary ?? ""));
					append($$anchor, span_5);
				};
				var consequent_11 = ($$anchor) => {
					var fragment_5 = comment();
					var node_7 = first_child(fragment_5);
					var consequent_10 = ($$anchor) => {
						var span_6 = root_7$20();
						let classes_2;
						var node_8 = child(span_6);
						var consequent_9 = ($$anchor) => {
							var fragment_6 = root_6$22();
							Icon(first_child(fragment_6), {
								name: "alert",
								size: 12
							});
							next();
							append($$anchor, fragment_6);
						};
						var d = /* @__PURE__ */ user_derived(() => crowded(r()));
						if_block(node_8, ($$render) => {
							if (get(d)) $$render(consequent_9);
						});
						var text_8 = sibling(node_8);
						reset(span_6);
						template_effect(($0, $1, $2) => {
							classes_2 = set_class(span_6, 1, "ctx svelte-13ck13b", null, classes_2, { hot: $0 });
							set_attribute(span_6, "title", $1);
							set_text(text_8, `${$2 ?? ""}%`);
						}, [
							() => crowded(r()),
							() => crowded(r()) ? "context nearly full" : void 0,
							() => Math.round(r().context_percent)
						]);
						append($$anchor, span_6);
					};
					var alternate = ($$anchor) => {
						append($$anchor, root_8$20());
					};
					if_block(node_7, ($$render) => {
						if (r().context_percent != null) $$render(consequent_10);
						else $$render(alternate, -1);
					});
					append($$anchor, fragment_5);
				};
				var consequent_13 = ($$anchor) => {
					var fragment_7 = comment();
					var node_10 = first_child(fragment_7);
					var consequent_12 = ($$anchor) => {
						append($$anchor, root_9$17());
					};
					var alternate_1 = ($$anchor) => {
						var text_9 = text();
						template_effect(($0) => set_text(text_9, `$${$0 ?? ""}`), [() => r().cost_usd.toFixed(2)]);
						append($$anchor, text_9);
					};
					if_block(node_10, ($$render) => {
						if (r().cost_unknown || r().cost_usd == null) $$render(consequent_12);
						else $$render(alternate_1, -1);
					});
					append($$anchor, fragment_7);
				};
				var consequent_14 = ($$anchor) => {
					var text_10 = text();
					template_effect(() => set_text(text_10, r().tool_calls ?? 0));
					append($$anchor, text_10);
				};
				var alternate_2 = ($$anchor) => {
					var text_11 = text();
					template_effect(($0) => set_text(text_11, $0), [() => when(r().last_event_at)]);
					append($$anchor, text_11);
				};
				if_block(node_5, ($$render) => {
					if (c().key === "state") $$render(consequent_5);
					else if (c().key === "session") $$render(consequent_6, 1);
					else if (c().key === "project") $$render(consequent_7, 2);
					else if (c().key === "doing") $$render(consequent_8, 3);
					else if (c().key === "context") $$render(consequent_11, 4);
					else if (c().key === "cost") $$render(consequent_13, 5);
					else if (c().key === "tools") $$render(consequent_14, 6);
					else $$render(alternate_2, -1);
				});
				append($$anchor, fragment_2);
			};
			const empty = ($$anchor) => {
				var p_1 = root_10$17();
				var text_12 = only_child(p_1, true);
				template_effect(() => set_text(text_12, get(only) ? `No session is ${get(only)}.` : "No session."));
				append($$anchor, p_1);
			};
			let $0 = /* @__PURE__ */ user_derived(() => get(groupBy) === "none" ? void 0 : get(groupBy) === "project" ? (r) => r.project_name ?? "no project" : bucket);
			Grid(node_4, {
				id: "sessions",
				get columns() {
					return columns;
				},
				get rows() {
					return get(shown);
				},
				key: (r) => r.id,
				get group() {
					return get($0);
				},
				label: "sessions",
				get selected() {
					return get(selected);
				},
				set selected($$value) {
					set(selected, $$value, true);
				},
				cell,
				empty,
				$$slots: {
					cell: true,
					empty: true
				}
			});
		}
		var node_11 = sibling(node_4, 2);
		var consequent_16 = ($$anchor) => {
			var aside = root_12$11();
			var header_1 = child(aside);
			var b_2 = child(header_1);
			var text_13 = only_child(b_2, true);
			var node_12 = sibling(b_2, 2);
			{
				let $0 = /* @__PURE__ */ user_derived(() => get(pick).state ?? "unknown");
				Pill(node_12, { get word() {
					return get($0);
				} });
			}
			var button_2 = sibling(node_12, 2);
			Icon(child(button_2), {
				name: "x",
				size: 13
			});
			reset(button_2);
			reset(header_1);
			var node_14 = sibling(header_1, 2);
			{
				let $0 = /* @__PURE__ */ user_derived(() => [
					{
						label: "Project",
						value: get(pick).project_name
					},
					{
						label: "Agent",
						value: get(pick).agent
					},
					{
						label: "Mode",
						value: get(pick).mode === "driven" ? "driven by Devplane" : "watched"
					},
					{
						label: "Permissions",
						value: get(pick).permission_mode,
						missing: "not reported"
					},
					{
						label: "Model",
						value: get(pick).model,
						missing: "not reported"
					},
					{
						label: "Branch",
						value: get(pick).branch,
						mono: true,
						missing: "none"
					},
					{
						label: "Directory",
						value: get(pick).cwd,
						mono: true
					},
					{
						label: "Plan",
						value: get(pick).plan_total ? `${get(pick).plan_done ?? 0} of ${get(pick).plan_total} steps done` : null,
						missing: "no plan reported"
					},
					{
						label: "Sent",
						value: get(pick).sent_says
					},
					{
						label: "Subagents",
						value: get(pick).subagents ? String(get(pick).subagents) : null,
						missing: "none"
					},
					{
						label: "Session",
						value: get(pick).id,
						mono: true
					}
				]);
				Props(node_14, { get rows() {
					return get($0);
				} });
			}
			var div_3 = sibling(node_14, 2);
			var button_3 = child(div_3);
			Icon(child(button_3), {
				name: "terminal",
				size: 13
			});
			next();
			reset(button_3);
			reset(div_3);
			var node_16 = sibling(div_3, 2);
			var consequent_15 = ($$anchor) => {
				var p_2 = root_11$13();
				var text_14 = only_child(p_2, true);
				template_effect(() => set_text(text_14, get(said)));
				append($$anchor, p_2);
			};
			if_block(node_16, ($$render) => {
				if (get(said)) $$render(consequent_15);
			});
			reset(aside);
			template_effect(() => set_text(text_13, get(pick).name ?? get(pick).agent ?? "session"));
			delegated("click", button_2, () => set(selected, null));
			delegated("click", button_3, () => copy(`devplane attach ${get(pick).id}`));
			append($$anchor, aside);
		};
		if_block(node_11, ($$render) => {
			if (get(pick)) $$render(consequent_16);
		});
		reset(div_2);
		append($$anchor, div_2);
	};
	if_block(node_3, ($$render) => {
		if (!loaded() && runs().length === 0) $$render(consequent_2);
		else if (runs().length === 0 && error()) $$render(consequent_3, 1);
		else if (runs().length === 0) $$render(consequent_4, 2);
		else $$render(alternate_3, -1);
	});
	reset(div);
	bind_select_value(select, () => get(groupBy), ($$value) => set(groupBy, $$value));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/board/index.ts
bind({
	surface: "global",
	combo: "g b",
	action: "go-board",
	label: "go to the sessions"
});
onAction("go-board", () => {
	go("#board");
	return true;
});
register({
	id: "board",
	icon: "sessions",
	title: "Sessions",
	heading: "Sessions",
	band: "happening",
	order: 0,
	holds: "run",
	status: (feed) => {
		const s = feed.board?.summary;
		if (!s) return [];
		return [{
			n: s.working ?? 0,
			word: "working",
			icon: "sessions"
		}, ...s.failed ? [{
			n: s.failed,
			word: "failed",
			icon: "alert",
			tone: "fail"
		}] : []];
	},
	count: (feed) => {
		return feed.board?.runs?.length ?? null;
	},
	select: (feed, focus) => {
		const b = feed.board;
		return {
			runs: b?.runs ?? [],
			summary: b?.summary,
			coverage: b?.coverage ?? null,
			thresholds: b?.thresholds ?? null,
			watching: b?.watching ?? null,
			focus,
			...phase(feed)
		};
	},
	component: Board
});
//#endregion
//#region src/surfaces/change/List.svelte
var root$37 = /* @__PURE__ */ from_html(`<div class="fail"><!></div>`);
var root_1$37 = /* @__PURE__ */ from_html(`<div class="skel svelte-3ehcwp"></div>`);
var root_2$30 = /* @__PURE__ */ from_html(`<p class="quiet svelte-3ehcwp">No change has been started on this machine. <button class="link svelte-3ehcwp">Start one</button> — an isolated worktree, an agent in it, and the project's own gates.</p>`);
var root_3$27 = /* @__PURE__ */ from_html(`<p class="quiet svelte-3ehcwp"> </p>`);
var root_4$25 = /* @__PURE__ */ from_html(`<span class="wait svelte-3ehcwp"> </span>`);
var root_5$22 = /* @__PURE__ */ from_html(`<span class="dim svelte-3ehcwp">in place</span>`);
var root_6$21 = /* @__PURE__ */ from_html(`<span class="branch svelte-3ehcwp"><!> </span>`);
var root_7$19 = /* @__PURE__ */ from_html(`<div class="row svelte-3ehcwp" role="option" tabindex="-1"><span class="title svelte-3ehcwp"> </span> <span class="meta svelte-3ehcwp"><!><!> <!> <!></span> <!></div>`);
var root_8$19 = /* @__PURE__ */ from_html(`<button class="group svelte-3ehcwp"><!> <!> <span> </span> <span class="n svelte-3ehcwp"> </span></button> <!>`, 1);
var root_9$16 = /* @__PURE__ */ from_html(`<button class="foot svelte-3ehcwp"> </button>`);
var root_10$16 = /* @__PURE__ */ from_html(`<div class="list svelte-3ehcwp"><header class="svelte-3ehcwp"><span class="t svelte-3ehcwp">Changes</span> <span class="n svelte-3ehcwp"> </span> <button class="icon svelte-3ehcwp" title="Start a new change (Alt+N)" aria-label="start a new change"><!></button></header> <label class="filter svelte-3ehcwp"><!> <input placeholder="Filter by title, branch, project, state" aria-label="filter the changes" class="svelte-3ehcwp"/></label> <div class="rows svelte-3ehcwp" role="listbox" aria-label="changes" tabindex="0"><!></div> <!></div>`);
function List$2($$anchor, $$props) {
	push($$props, true);
	let all = prop($$props, "all", 19, () => []), focus = prop($$props, "focus", 3, ""), planted = prop($$props, "rows", 3, null);
	const read = resource(() => "/api/changes", { tell: () => "devplane change list" });
	const rows = /* @__PURE__ */ user_derived(() => Array.isArray(read.data) ? read.data : planted());
	let filter = /* @__PURE__ */ state("");
	let showArchived = /* @__PURE__ */ state(false);
	const signature = /* @__PURE__ */ user_derived(() => all().map((b) => `${b.id}:${b.state}`).join("|"));
	let lastSignature = null;
	user_effect(() => {
		const sig = get(signature);
		untrack(() => {
			if (lastSignature !== null && sig !== lastSignature) read.reload();
			lastSignature = sig;
		});
	});
	const name = (p) => p.split("/").filter(Boolean).pop() ?? p;
	const visible = /* @__PURE__ */ user_derived(() => (get(rows) ?? []).filter((r) => (get(showArchived) || !r.archived_at) && (!get(filter).trim() || `${r.title} ${r.branch ?? ""} ${name(r.project_id)} ${r.state}`.toLowerCase().includes(get(filter).trim().toLowerCase()))));
	const groups = /* @__PURE__ */ user_derived(() => {
		const m = /* @__PURE__ */ new Map();
		for (const r of get(visible)) {
			const k = name(r.project_id);
			m.set(k, [...m.get(k) ?? [], r]);
		}
		return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0]));
	});
	const archived = /* @__PURE__ */ user_derived(() => (get(rows) ?? []).filter((r) => r.archived_at).length);
	let folded = /* @__PURE__ */ state(proxy({}));
	function key(e) {
		const flat = get(groups).flatMap(([g, rs]) => get(folded)[g] ? [] : rs);
		if (flat.length === 0) return;
		const i = flat.findIndex((r) => r.id === focus());
		let next = i;
		if (e.key === "ArrowDown" || e.key === "j") next = Math.min(flat.length - 1, i + 1);
		else if (e.key === "ArrowUp" || e.key === "k") next = Math.max(0, i - 1);
		else if (e.key === "Enter" && i !== -1) {
			e.stopPropagation();
			$$props.open(flat[i].id, true);
			return;
		} else return;
		e.preventDefault();
		e.stopPropagation();
		$$props.open(flat[next].id);
	}
	const optionId = (id) => `change-row-${id.replace(/[^\w-]/g, "_")}`;
	const active = /* @__PURE__ */ user_derived(() => get(visible).some((r) => r.id === focus()) ? optionId(focus()) : void 0);
	var div = root_10$16();
	var header = child(div);
	var span = sibling(child(header), 2);
	var text = only_child(span, true);
	var button = sibling(span, 2);
	Icon(child(button), {
		name: "plus",
		size: 15
	});
	reset(button);
	reset(header);
	var label = sibling(header, 2);
	var node_1 = child(label);
	Icon(node_1, {
		name: "filter",
		size: 13
	});
	var input = sibling(node_1, 2);
	remove_input_defaults(input);
	reset(label);
	var div_1 = sibling(label, 2);
	var node_2 = child(div_1);
	var consequent = ($$anchor) => {
		var div_2 = root$37();
		var node_3 = child(div_2);
		{
			let $0 = /* @__PURE__ */ user_derived(() => read.data !== null);
			Failed(node_3, {
				what: "the changes",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				get stale() {
					return get($0);
				}
			});
		}
		reset(div_2);
		append($$anchor, div_2);
	};
	var consequent_1 = ($$anchor) => {
		var fragment = comment();
		each(first_child(fragment), 16, () => [
			0,
			1,
			2
		], (i) => i, ($$anchor, i) => {
			append($$anchor, root_1$37());
		});
		append($$anchor, fragment);
	};
	var consequent_2 = ($$anchor) => {
		var p_1 = root_2$30();
		var button_1 = sibling(child(p_1));
		next();
		reset(p_1);
		delegated("click", button_1, () => run("new-change", "change"));
		append($$anchor, p_1);
	};
	var consequent_3 = ($$anchor) => {
		var p_2 = root_3$27();
		var text_1 = only_child(p_2);
		template_effect(() => set_text(text_1, `No change matches “${get(filter) ?? ""}”.`));
		append($$anchor, p_2);
	};
	var alternate = ($$anchor) => {
		var fragment_1 = comment();
		each(first_child(fragment_1), 17, () => get(groups), ([g, rs]) => g, ($$anchor, $$item) => {
			var $$array = /* @__PURE__ */ user_derived(() => to_array(get($$item), 2));
			let g = () => get($$array)[0];
			let rs = () => get($$array)[1];
			var fragment_2 = root_8$19();
			var button_2 = first_child(fragment_2);
			var node_6 = child(button_2);
			{
				let $0 = /* @__PURE__ */ user_derived(() => get(folded)[g()] ? "right" : "down");
				Icon(node_6, {
					get name() {
						return get($0);
					},
					size: 12
				});
			}
			var node_7 = sibling(node_6, 2);
			Icon(node_7, {
				name: "folder",
				size: 13
			});
			var span_1 = sibling(node_7, 2);
			var text_2 = only_child(span_1, true);
			var text_3 = only_child(sibling(span_1, 2), true);
			reset(button_2);
			var node_8 = sibling(button_2, 2);
			var consequent_7 = ($$anchor) => {
				var fragment_3 = comment();
				each(first_child(fragment_3), 17, rs, (r) => r.id, ($$anchor, r) => {
					var div_4 = root_7$19();
					var span_3 = child(div_4);
					var text_4 = only_child(span_3, true);
					var span_4 = sibling(span_3, 2);
					var node_10 = child(span_4);
					Pill(node_10, { get word() {
						return get(r).state;
					} });
					var node_11 = sibling(node_10);
					Qualifier(node_11, { get q() {
						return get(r).qualifier;
					} });
					var node_12 = sibling(node_11, 2);
					var consequent_4 = ($$anchor) => {
						var span_5 = root_4$25();
						var text_5 = only_child(span_5, true);
						template_effect(() => set_text(text_5, get(r).waiting_says));
						append($$anchor, span_5);
					};
					if_block(node_12, ($$render) => {
						if (get(r).waiting_says) $$render(consequent_4);
					});
					var node_13 = sibling(node_12, 2);
					var consequent_5 = ($$anchor) => {
						append($$anchor, root_5$22());
					};
					if_block(node_13, ($$render) => {
						if (get(r).in_place) $$render(consequent_5);
					});
					reset(span_4);
					var node_14 = sibling(span_4, 2);
					var consequent_6 = ($$anchor) => {
						var span_7 = root_6$21();
						var node_15 = child(span_7);
						Icon(node_15, {
							name: "change",
							size: 11
						});
						var text_6 = sibling(node_15);
						reset(span_7);
						template_effect(() => set_text(text_6, ` ${get(r).branch ?? ""}`));
						append($$anchor, span_7);
					};
					if_block(node_14, ($$render) => {
						if (get(r).branch) $$render(consequent_6);
					});
					reset(div_4);
					template_effect(($0) => {
						set_attribute(div_4, "id", $0);
						set_attribute(div_4, "aria-selected", get(r).id === focus());
						set_text(text_4, get(r).title || get(r).id);
					}, [() => optionId(get(r).id)]);
					delegated("click", div_4, () => $$props.open(get(r).id));
					delegated("dblclick", div_4, () => $$props.open(get(r).id, true));
					delegated("keydown", div_4, (e) => {
						if (e.key === "Enter" || e.key === " ") {
							e.preventDefault();
							e.stopPropagation();
							$$props.open(get(r).id, e.key === "Enter");
						}
					});
					append($$anchor, div_4);
				});
				append($$anchor, fragment_3);
			};
			if_block(node_8, ($$render) => {
				if (!get(folded)[g()]) $$render(consequent_7);
			});
			template_effect(() => {
				set_attribute(button_2, "aria-expanded", !get(folded)[g()]);
				set_text(text_2, g());
				set_text(text_3, rs().length);
			});
			delegated("click", button_2, () => set(folded, {
				...get(folded),
				[g()]: !get(folded)[g()]
			}, true));
			append($$anchor, fragment_2);
		});
		append($$anchor, fragment_1);
	};
	if_block(node_2, ($$render) => {
		if (read.failure) $$render(consequent);
		else if (get(rows) === null) $$render(consequent_1, 1);
		else if (get(rows).length === 0) $$render(consequent_2, 2);
		else if (get(visible).length === 0) $$render(consequent_3, 3);
		else $$render(alternate, -1);
	});
	reset(div_1);
	var node_16 = sibling(div_1, 2);
	var consequent_8 = ($$anchor) => {
		var button_3 = root_9$16();
		var text_7 = only_child(button_3);
		template_effect(() => set_text(text_7, `${get(showArchived) ? "Hide" : "Show"} ${get(archived) ?? ""} archived`));
		delegated("click", button_3, () => set(showArchived, !get(showArchived)));
		append($$anchor, button_3);
	};
	if_block(node_16, ($$render) => {
		if (get(archived) > 0) $$render(consequent_8);
	});
	reset(div);
	template_effect(() => {
		set_text(text, get(rows) === null ? "" : get(visible).length);
		set_attribute(div_1, "aria-activedescendant", get(active));
	});
	delegated("click", button, () => run("new-change", "change"));
	bind_value(input, () => get(filter), ($$value) => set(filter, $$value));
	delegated("keydown", div_1, key);
	append($$anchor, div);
	pop();
}
delegate([
	"click",
	"keydown",
	"dblclick"
]);
//#endregion
//#region src/lib/md.ts
function inline(text) {
	if (!text) return [];
	const out = [];
	for (const p of text.split(/(`[^`\n]+`)/g)) {
		if (!p) continue;
		if (p.length > 2 && p.startsWith("`") && p.endsWith("`")) {
			out.push({
				k: "code",
				s: p.slice(1, -1)
			});
			continue;
		}
		for (const q of p.split(/(\*\*[^*\n]+\*\*)/g)) {
			if (!q) continue;
			if (q.length > 4 && q.startsWith("**") && q.endsWith("**")) out.push({
				k: "strong",
				s: q.slice(2, -2)
			});
			else out.push({
				k: "t",
				s: q
			});
		}
	}
	return out;
}
function blocks(text) {
	const out = [];
	let fence = null;
	let para = [];
	const flush = () => {
		if (para.length) out.push({
			kind: "p",
			segs: inline(para.join(" "))
		});
		para = [];
	};
	for (const line of (text ?? "").split("\n")) {
		if (line.trimStart().startsWith("```")) {
			if (fence) {
				out.push({
					kind: "pre",
					text: fence.join("\n")
				});
				fence = null;
			} else {
				flush();
				fence = [];
			}
			continue;
		}
		if (fence) {
			fence.push(line);
			continue;
		}
		const t = line.trim();
		if (!t) flush();
		else if (/^#{1,6}\s/.test(t)) {
			flush();
			out.push({
				kind: "h",
				segs: inline(t.replace(/^#+\s*/, ""))
			});
		} else if (/^[-*]\s/.test(t)) {
			flush();
			out.push({
				kind: "li",
				segs: inline(t.slice(2))
			});
		} else if (t.startsWith("|")) {
			flush();
			if (!/^\|[\s|:-]+\|$/.test(t)) {
				const cells = t.replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
				out.push({
					kind: "row",
					segs: inline(cells.join("  ·  "))
				});
			}
		} else para.push(t);
	}
	if (fence) out.push({
		kind: "pre",
		text: fence.join("\n")
	});
	flush();
	return out;
}
//#endregion
//#region src/lib/ui/Inline.svelte
var root$36 = /* @__PURE__ */ from_html(`<code> </code>`);
var root_1$36 = /* @__PURE__ */ from_html(`<strong> </strong>`);
function Inline($$anchor, $$props) {
	push($$props, true);
	let text$5 = prop($$props, "text", 3, null), segs = prop($$props, "segs", 3, null);
	const parts = /* @__PURE__ */ user_derived(() => segs() ?? inline(text$5()));
	var fragment = comment();
	each(first_child(fragment), 17, () => get(parts), index, ($$anchor, p) => {
		var fragment_1 = comment();
		var node_1 = first_child(fragment_1);
		var consequent = ($$anchor) => {
			var code = root$36();
			var text_1 = only_child(code, true);
			template_effect(() => set_text(text_1, get(p).s));
			append($$anchor, code);
		};
		var consequent_1 = ($$anchor) => {
			var strong = root_1$36();
			var text_2 = only_child(strong, true);
			template_effect(() => set_text(text_2, get(p).s));
			append($$anchor, strong);
		};
		var alternate = ($$anchor) => {
			var text_3 = text();
			template_effect(() => set_text(text_3, get(p).s));
			append($$anchor, text_3);
		};
		if_block(node_1, ($$render) => {
			if (get(p).k === "code") $$render(consequent);
			else if (get(p).k === "strong") $$render(consequent_1, 1);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment_1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
//#region src/lib/ui/Tabs.svelte
function step(ids, active, key) {
	if (ids.length === 0) return null;
	const i = Math.max(0, ids.indexOf(active));
	if (key === "ArrowRight") return ids[(i + 1) % ids.length];
	if (key === "ArrowLeft") return ids[(i - 1 + ids.length) % ids.length];
	if (key === "Home") return ids[0];
	if (key === "End") return ids[ids.length - 1];
	return null;
}
var root$35 = /* @__PURE__ */ from_html(`<span> </span>`);
var root_1$35 = /* @__PURE__ */ from_html(`<button role="tab" class="tab svelte-1fwbyla"><!> <span> </span> <!></button>`);
var root_2$29 = /* @__PURE__ */ from_html(`<div class="tabs svelte-1fwbyla" role="tablist" tabindex="-1"></div>`);
function Tabs($$anchor, $$props) {
	push($$props, true);
	let active = prop($$props, "active", 15), label = prop($$props, "label", 3, "views");
	function key(e) {
		const to = step($$props.tabs.map((t) => t.id), active(), e.key);
		if (to === null) return;
		e.preventDefault();
		active(to);
		const next = $$props.tabs.findIndex((t) => t.id === to);
		e.currentTarget.querySelectorAll("[role=tab]")[next]?.focus();
	}
	var div = root_2$29();
	each(div, 21, () => $$props.tabs, (t) => t.id, ($$anchor, t) => {
		var button = root_1$35();
		var node = child(button);
		var consequent = ($$anchor) => {
			Icon($$anchor, {
				get name() {
					return get(t).icon;
				},
				size: 14
			});
		};
		if_block(node, ($$render) => {
			if (get(t).icon) $$render(consequent);
		});
		var span = sibling(node, 2);
		var text = only_child(span, true);
		var node_1 = sibling(span, 2);
		var consequent_1 = ($$anchor) => {
			var span_1 = root$35();
			var text_1 = only_child(span_1, true);
			template_effect(() => {
				set_class(span_1, 1, `count ${get(t).tone ?? "" ?? ""}`, "svelte-1fwbyla");
				set_text(text_1, get(t).count);
			});
			append($$anchor, span_1);
		};
		if_block(node_1, ($$render) => {
			if (get(t).count != null) $$render(consequent_1);
		});
		reset(button);
		template_effect(() => {
			set_attribute(button, "aria-selected", get(t).id === active());
			set_attribute(button, "tabindex", get(t).id === active() ? 0 : -1);
			set_text(text, get(t).label);
		});
		delegated("click", button, () => active(get(t).id));
		append($$anchor, button);
	});
	reset(div);
	template_effect(() => set_attribute(div, "aria-label", label()));
	delegated("keydown", div, key);
	append($$anchor, div);
	pop();
}
delegate(["keydown", "click"]);
//#endregion
//#region src/surfaces/change/Stepper.svelte
var root$34 = /* @__PURE__ */ from_html(`<li><span class="node svelte-bllsbm" aria-hidden="true"></span> <span class="label svelte-bllsbm"> </span></li>`);
var root_1$34 = /* @__PURE__ */ from_html(`<ol class="stepper svelte-bllsbm" aria-label="where this change is"></ol>`);
function Stepper($$anchor, $$props) {
	push($$props, true);
	const VERIFIED = LIFE.indexOf(STATES.verified.word);
	let archived = prop($$props, "archived", 3, false), offered = prop($$props, "offered", 3, false);
	const at = /* @__PURE__ */ user_derived(() => {
		if (archived()) return LIFE.length - 1;
		if (offered()) return LIFE.length - 2;
		const i = LIFE.indexOf($$props.state);
		return i === -1 ? 2 : i;
	});
	const off = /* @__PURE__ */ user_derived(() => !LIFE.includes($$props.state) && !archived() && !offered());
	var ol = root_1$34();
	each(ol, 22, () => LIFE, (s) => s, ($$anchor, s, i) => {
		var li = root$34();
		let classes;
		var text = only_child(sibling(child(li), 2), true);
		reset(li);
		template_effect(() => {
			set_attribute(li, "aria-current", get(i) === get(at) ? "step" : void 0);
			classes = set_class(li, 1, "svelte-bllsbm", null, classes, {
				past: get(i) < get(at),
				here: get(i) === get(at),
				verified: get(i) === get(at) && get(i) === VERIFIED,
				off: get(i) === get(at) && get(off)
			});
			set_text(text, get(i) === get(at) && get(off) ? $$props.state : STATES[s].word);
		});
		append($$anchor, li);
	});
	reset(ol);
	append($$anchor, ol);
	pop();
}
//#endregion
//#region src/lib/ui/Timeline.svelte
var root$33 = /* @__PURE__ */ from_html(`<p class="none svelte-hwm3dn">Nothing has happened to it yet.</p>`);
var root_1$33 = /* @__PURE__ */ from_html(`<button></button>`);
var root_2$28 = /* @__PURE__ */ from_html(`<div class="lane svelte-hwm3dn"><span class="name svelte-hwm3dn"> </span> <div class="track svelte-hwm3dn"><!> <span class="now svelte-hwm3dn" aria-hidden="true"></span></div></div>`);
var root_3$26 = /* @__PURE__ */ from_html(`<figure class="tl svelte-hwm3dn"><!> <div class="axis svelte-hwm3dn"><span></span><span class="ticks svelte-hwm3dn"><span> </span><span> </span></span></div> <figcaption aria-live="polite" class="svelte-hwm3dn"> </figcaption></figure>`);
function Timeline($$anchor, $$props) {
	push($$props, true);
	const times = /* @__PURE__ */ user_derived(() => $$props.marks.map((m) => new Date(m.at).getTime()).filter((t) => !Number.isNaN(t)));
	const lo = /* @__PURE__ */ user_derived(() => get(times).length ? Math.min(...get(times)) : 0);
	const hi = /* @__PURE__ */ user_derived(() => get(times).length ? Math.max(Date.now(), ...get(times)) : 1);
	const span = /* @__PURE__ */ user_derived(() => Math.max(1, get(hi) - get(lo)));
	const pct = (at) => (new Date(at).getTime() - get(lo)) / get(span) * 100;
	function clock(t) {
		const d = new Date(t);
		return (/* @__PURE__ */ new Date()).toDateString() === d.toDateString() ? d.toTimeString().slice(0, 5) : `${d.toLocaleDateString()} ${d.toTimeString().slice(0, 5)}`;
	}
	let hover = /* @__PURE__ */ state(null);
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		append($$anchor, root$33());
	};
	var alternate = ($$anchor) => {
		var figure = root_3$26();
		var node_1 = child(figure);
		each(node_1, 16, () => $$props.lanes, (lane) => lane, ($$anchor, lane) => {
			var div = root_2$28();
			var span_1 = child(div);
			var text = only_child(span_1, true);
			var div_1 = sibling(span_1, 2);
			each(child(div_1), 17, () => $$props.marks.filter((m) => m.lane === lane), index, ($$anchor, m) => {
				var button = root_1$33();
				let styles;
				template_effect(($0, $1, $2) => {
					set_class(button, 1, `mark ${get(m).tone ?? "none" ?? ""}`, "svelte-hwm3dn");
					set_attribute(button, "title", `${get(m).label ?? ""} — ${$0 ?? ""}`);
					set_attribute(button, "aria-label", `${get(m).label ?? ""} at ${$1 ?? ""}`);
					styles = set_style(button, "", styles, { left: $2 });
				}, [
					() => new Date(get(m).at).toLocaleString(),
					() => new Date(get(m).at).toLocaleString(),
					() => `${pct(get(m).at) ?? ""}%`
				]);
				event("mouseenter", button, () => set(hover, get(m), true));
				event("mouseleave", button, () => set(hover, null));
				event("focus", button, () => set(hover, get(m), true));
				event("blur", button, () => set(hover, null));
				append($$anchor, button);
			});
			next(2);
			reset(div_1);
			reset(div);
			template_effect(() => set_text(text, lane));
			append($$anchor, div);
		});
		var div_2 = sibling(node_1, 2);
		var span_2 = sibling(child(div_2));
		var span_3 = child(span_2);
		var text_1 = only_child(span_3, true);
		var text_2 = only_child(sibling(span_3));
		reset(span_2);
		reset(div_2);
		var text_3 = only_child(sibling(div_2, 2), true);
		reset(figure);
		template_effect(($0, $1) => {
			set_attribute(figure, "aria-label", `timeline of ${$$props.marks.length ?? ""} events`);
			set_text(text_1, $0);
			set_text(text_2, `now · ${$1 ?? ""}`);
			set_text(text_3, get(hover) ? `${get(hover).lane} — ${get(hover).label}` : " ");
		}, [() => clock(get(lo)), () => clock(get(hi))]);
		append($$anchor, figure);
	};
	if_block(node, ($$render) => {
		if ($$props.marks.length === 0) $$render(consequent);
		else $$render(alternate, -1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
//#region src/surfaces/change/types.ts
function exit(o) {
	if (o.outcome === "exited") return `exit ${o.code}`;
	if (o.outcome === "timed_out") return `timed out after ${o.after_secs}s`;
	return `${o.outcome.replace(/_/g, " ")}${o.reason ? `: ${o.reason}` : ""}`;
}
function took(ms) {
	if (ms < 1e3) return `${ms} ms`;
	if (ms < 6e4) return `${(ms / 1e3).toFixed(1)} s`;
	return `${Math.floor(ms / 6e4)} m ${Math.round(ms % 6e4 / 1e3)} s`;
}
function ago(at) {
	if (!at) return "—";
	const t = new Date(at).getTime();
	if (Number.isNaN(t)) return "—";
	const s = Math.max(0, Math.round((Date.now() - t) / 1e3));
	if (s < 60) return `${s}s ago`;
	if (s < 3600) return `${Math.round(s / 60)}m ago`;
	if (s < 86400) return `${Math.round(s / 3600)}h ago`;
	return `${Math.round(s / 86400)}d ago`;
}
//#endregion
//#region src/surfaces/change/Overview.svelte
var root$32 = /* @__PURE__ */ from_html(`<span class="v quiet svelte-1og7e16">No gate has run against this change.</span>`);
var root_1$32 = /* @__PURE__ */ from_html(`<span> </span>`);
var root_2$27 = /* @__PURE__ */ from_html(`<code class="cmd svelte-1og7e16"> </code>`);
var root_3$25 = /* @__PURE__ */ from_html(`<span class="v fail svelte-1og7e16"> </span> <!>`, 1);
var root_4$24 = /* @__PURE__ */ from_html(`<b> </b> seen by a passing check`, 1);
var root_5$21 = /* @__PURE__ */ from_html(`<span class="s svelte-1og7e16"> </span>`);
var root_6$20 = /* @__PURE__ */ from_html(`<span class="s wait svelte-1og7e16"> </span>`);
var root_7$18 = /* @__PURE__ */ from_html(`<span class="v svelte-1og7e16"><b> </b> tasks · <b> </b> ticked · <!></span> <!> <!>`, 1);
var root_8$18 = /* @__PURE__ */ from_html(`<span class="v quiet svelte-1og7e16"> </span>`);
var root_9$15 = /* @__PURE__ */ from_html(`<span class="v fail svelte-1og7e16"> </span> <span class="s svelte-1og7e16"><code> </code> tells more</span>`, 1);
var root_10$15 = /* @__PURE__ */ from_html(`<span class="v quiet svelte-1og7e16">Reading…</span>`);
var root_11$12 = /* @__PURE__ */ from_html(`<span class="v quiet svelte-1og7e16">Nothing has been decided about it.</span>`);
var root_12$10 = /* @__PURE__ */ from_html(`<span class="v svelte-1og7e16"><b></b> read — there are more</span> <span class="s svelte-1og7e16"><code> </code> has every one</span>`, 1);
var root_13$10 = /* @__PURE__ */ from_html(`<span class="v svelte-1og7e16"><b> </b> recorded</span> <span class="s svelte-1og7e16"> </span>`, 1);
var root_14$10 = /* @__PURE__ */ from_html(`<span class="s fail svelte-1og7e16"> </span>`);
var root_15$8 = /* @__PURE__ */ from_html(`<p class="svelte-1og7e16"> </p>`);
var root_16$5 = /* @__PURE__ */ from_html(`<section class="alert svelte-1og7e16"><!> <div><b>The specification moved under a run.</b> <!> <button class="svelte-1og7e16">Decide in Tasks</button></div></section>`);
var root_17$3 = /* @__PURE__ */ from_html(`<div class="report svelte-1og7e16"><span>to <b> </b> </span> <span class="quiet svelte-1og7e16"> </span> <pre class="svelte-1og7e16"> </pre></div>`);
var root_18$1 = /* @__PURE__ */ from_html(`<section class="reports svelte-1og7e16"><h2 class="svelte-1og7e16">Reports filed <span class="n svelte-1og7e16"> </span></h2> <!></section>`);
var root_19$1 = /* @__PURE__ */ from_html(`<div class="grid svelte-1og7e16"><section class="cards svelte-1og7e16"><button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Gates</span> <!></button> <button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Tasks</span> <!></button> <button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Decisions</span> <!></button> <button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Agent</span> <span class="v svelte-1og7e16"><b> </b> <!></span> <!></button></section> <!> <section class="facts svelte-1og7e16"><h2 class="svelte-1og7e16">Facts</h2> <!></section> <section class="history svelte-1og7e16"><h2 class="svelte-1og7e16">History</h2> <!> <!></section> <!></div>`);
function Overview($$anchor, $$props) {
	push($$props, true);
	const LIMIT = 200;
	const about = /* @__PURE__ */ user_derived(() => $$props.d.id);
	const read = resource(() => `/api/decisions?about=${encodeURIComponent(get(about))}&limit=${LIMIT}`, { tell: () => `devplane audit ${get(about)}` });
	const decisions = /* @__PURE__ */ user_derived(() => Array.isArray(read.data) ? read.data : []);
	const failing = /* @__PURE__ */ user_derived(() => $$props.d.gate && !$$props.d.gate.passed ? ($$props.d.gate.commands ?? []).filter((c) => !(c.outcome.outcome === "exited" && c.outcome.code === 0)) : []);
	const short = (s) => s ? s.slice(0, 10) : null;
	const tone = (a) => a === "person" ? "work" : a === "rule" ? "wait" : a === "timer" || a === "nobody" ? "fail" : "none";
	const marks = /* @__PURE__ */ user_derived(() => [
		...$$props.d.created_at ? [{
			at: $$props.d.created_at,
			lane: "change",
			label: "started",
			tone: "work"
		}] : [],
		...($$props.d.gates ?? []).map((g) => {
			const ok = g.commands.every((c) => c.outcome.outcome === "exited" && c.outcome.code === 0);
			return {
				at: g.at,
				lane: "gates",
				label: `${g.gate} attempt ${g.attempt}: ${ok ? "passed" : "failed"}`,
				tone: ok ? "done" : "fail"
			};
		}),
		...get(decisions).map((x) => ({
			at: x.at,
			lane: "decisions",
			label: `${x.authority} · ${x.action} · ${x.outcome}`,
			tone: tone(x.authority)
		})),
		...$$props.d.archived_at ? [{
			at: $$props.d.archived_at,
			lane: "change",
			label: STATES.archived.word,
			tone: "none"
		}] : []
	]);
	const byAuthority = /* @__PURE__ */ user_derived(() => Object.entries(get(decisions).reduce((m, x) => (m[x.authority] = (m[x.authority] ?? 0) + 1, m), {})));
	var div = root_19$1();
	var section = child(div);
	var button = child(section);
	var span = child(button);
	Icon(child(span), {
		name: "gate",
		size: 14
	});
	next();
	reset(span);
	var node_1 = sibling(span, 2);
	var consequent = ($$anchor) => {
		append($$anchor, root$32());
	};
	var consequent_1 = ($$anchor) => {
		var span_2 = root_1$32();
		let classes;
		var text = only_child(span_2);
		template_effect(() => {
			classes = set_class(span_2, 1, "v svelte-1og7e16", null, classes, { done: $$props.d.state === STATES.verified.word });
			set_text(text, `${$$props.d.gate.name ?? ""} passed · attempt ${$$props.d.gate.attempt ?? ""}`);
		});
		append($$anchor, span_2);
	};
	var alternate = ($$anchor) => {
		var fragment = root_3$25();
		var span_3 = first_child(fragment);
		var text_1 = only_child(span_3);
		each(sibling(span_3, 2), 17, () => get(failing).slice(0, 2), (c) => c.command, ($$anchor, c) => {
			var code = root_2$27();
			var text_2 = only_child(code);
			template_effect(($0) => set_text(text_2, `${get(c).command ?? ""} — ${$0 ?? ""}`), [() => exit(get(c).outcome)]);
			append($$anchor, code);
		});
		template_effect(() => set_text(text_1, `${$$props.d.gate.name ?? ""} failed · ${get(failing).length ?? ""} of ${$$props.d.gate.commands?.length ?? 0 ?? ""} commands`));
		append($$anchor, fragment);
	};
	if_block(node_1, ($$render) => {
		if (!$$props.d.gate) $$render(consequent);
		else if ($$props.d.gate.passed) $$render(consequent_1, 1);
		else $$render(alternate, -1);
	});
	reset(button);
	var button_1 = sibling(button, 2);
	var span_4 = child(button_1);
	Icon(child(span_4), {
		name: "spec",
		size: 14
	});
	next();
	reset(span_4);
	var node_4 = sibling(span_4, 2);
	var consequent_5 = ($$anchor) => {
		var fragment_1 = root_7$18();
		var span_5 = first_child(fragment_1);
		var b = child(span_5);
		var text_3 = only_child(b, true);
		var b_1 = sibling(b, 2);
		var text_4 = only_child(b_1, true);
		var node_5 = sibling(b_1, 2);
		var consequent_2 = ($$anchor) => {
			append($$anchor, text("no gates declared"));
		};
		var alternate_1 = ($$anchor) => {
			var fragment_2 = root_4$24();
			var text_6 = only_child(first_child(fragment_2), true);
			next();
			template_effect(() => set_text(text_6, $$props.d.counts.seen_by_pass));
			append($$anchor, fragment_2);
		};
		if_block(node_5, ($$render) => {
			if ($$props.d.counts.seen_by_pass == null) $$render(consequent_2);
			else $$render(alternate_1, -1);
		});
		reset(span_5);
		var node_6 = sibling(span_5, 2);
		var consequent_3 = ($$anchor) => {
			var span_6 = root_5$21();
			var text_7 = only_child(span_6, true);
			template_effect(() => set_text(text_7, $$props.d.counts_says));
			append($$anchor, span_6);
		};
		if_block(node_6, ($$render) => {
			if ($$props.d.counts_says) $$render(consequent_3);
		});
		var node_7 = sibling(node_6, 2);
		var consequent_4 = ($$anchor) => {
			var span_7 = root_6$20();
			var text_8 = only_child(span_7);
			template_effect(() => set_text(text_8, `${$$props.d.counts.ticked_unsent.length ?? ""} ticked that no run was sent`));
			append($$anchor, span_7);
		};
		if_block(node_7, ($$render) => {
			if ($$props.d.counts.ticked_unsent?.length) $$render(consequent_4);
		});
		template_effect(() => {
			set_text(text_3, $$props.d.counts.tasks);
			set_text(text_4, $$props.d.counts.ticked);
		});
		append($$anchor, fragment_1);
	};
	var alternate_2 = ($$anchor) => {
		var span_8 = root_8$18();
		var text_9 = only_child(span_8, true);
		template_effect(() => set_text(text_9, $$props.d.spec ? "The specification has no tasks." : "No specification is attached."));
		append($$anchor, span_8);
	};
	if_block(node_4, ($$render) => {
		if ($$props.d.counts) $$render(consequent_5);
		else $$render(alternate_2, -1);
	});
	reset(button_1);
	var button_2 = sibling(button_1, 2);
	var span_9 = child(button_2);
	Icon(child(span_9), {
		name: "ledger",
		size: 14
	});
	next();
	reset(span_9);
	var node_9 = sibling(span_9, 2);
	var consequent_6 = ($$anchor) => {
		var fragment_3 = root_9$15();
		var span_10 = first_child(fragment_3);
		var text_10 = only_child(span_10);
		var span_11 = sibling(span_10, 2);
		var text_11 = only_child(child(span_11), true);
		next();
		reset(span_11);
		template_effect(() => {
			set_text(text_10, `Not read: ${read.failure.says ?? ""}`);
			set_text(text_11, read.failure.tell);
		});
		append($$anchor, fragment_3);
	};
	var consequent_7 = ($$anchor) => {
		append($$anchor, root_10$15());
	};
	var consequent_8 = ($$anchor) => {
		append($$anchor, root_11$12());
	};
	var consequent_9 = ($$anchor) => {
		var fragment_4 = root_12$10();
		var span_14 = first_child(fragment_4);
		var b_3 = child(span_14);
		b_3.textContent = "200";
		next();
		reset(span_14);
		var span_15 = sibling(span_14, 2);
		var text_12 = only_child(child(span_15));
		next();
		reset(span_15);
		template_effect(() => set_text(text_12, `devplane audit ${get(about) ?? ""}`));
		append($$anchor, fragment_4);
	};
	var alternate_3 = ($$anchor) => {
		var fragment_5 = root_13$10();
		var span_16 = first_child(fragment_5);
		var text_13 = only_child(child(span_16), true);
		next();
		reset(span_16);
		var text_14 = only_child(sibling(span_16, 2), true);
		template_effect(($0) => {
			set_text(text_13, get(decisions).length);
			set_text(text_14, $0);
		}, [() => get(byAuthority).map(([a, n]) => `${n} by ${a}`).join(" · ")]);
		append($$anchor, fragment_5);
	};
	if_block(node_9, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent_6);
		else if (read.phase === "loading") $$render(consequent_7, 1);
		else if (get(decisions).length === 0) $$render(consequent_8, 2);
		else if (get(decisions).length >= LIMIT) $$render(consequent_9, 3);
		else $$render(alternate_3, -1);
	});
	reset(button_2);
	var button_3 = sibling(button_2, 2);
	var span_18 = child(button_3);
	Icon(child(span_18), {
		name: "agent",
		size: 14
	});
	next();
	reset(span_18);
	var span_19 = sibling(span_18, 2);
	var b_5 = child(span_19);
	var text_15 = only_child(b_5, true);
	var text_16 = sibling(b_5);
	var node_11 = sibling(text_16);
	var consequent_10 = ($$anchor) => {
		var text_17 = text();
		template_effect(() => set_text(text_17, `· ${$$props.d.feedback_rounds ?? ""} handed back`));
		append($$anchor, text_17);
	};
	if_block(node_11, ($$render) => {
		if ($$props.d.feedback_rounds) $$render(consequent_10);
	});
	reset(span_19);
	var node_12 = sibling(span_19, 2);
	var consequent_11 = ($$anchor) => {
		var span_20 = root_14$10();
		var text_18 = only_child(span_20, true);
		template_effect(() => set_text(text_18, $$props.d.stopped_summary));
		append($$anchor, span_20);
	};
	if_block(node_12, ($$render) => {
		if ($$props.d.stopped_summary) $$render(consequent_11);
	});
	reset(button_3);
	reset(section);
	var node_13 = sibling(section, 2);
	var consequent_12 = ($$anchor) => {
		var section_1 = root_16$5();
		var node_14 = child(section_1);
		Icon(node_14, {
			name: "alert",
			size: 16
		});
		var div_1 = sibling(node_14, 2);
		var node_15 = sibling(child(div_1), 2);
		each(node_15, 17, () => $$props.d.drifts ?? [], index, ($$anchor, x) => {
			var p = root_15$8();
			var text_19 = only_child(p, true);
			template_effect(() => set_text(text_19, get(x).says));
			append($$anchor, p);
		});
		var button_4 = sibling(node_15, 2);
		reset(div_1);
		reset(section_1);
		delegated("click", button_4, () => $$props.go("tasks"));
		append($$anchor, section_1);
	};
	if_block(node_13, ($$render) => {
		if (($$props.d.drifts ?? []).length > 0) $$render(consequent_12);
	});
	var section_2 = sibling(node_13, 2);
	var node_16 = sibling(child(section_2), 2);
	{
		let $0 = /* @__PURE__ */ user_derived(() => [
			{
				label: "Branch",
				value: $$props.d.branch,
				mono: true,
				missing: "none — works in place"
			},
			{
				label: "Worktree",
				value: $$props.d.worktree,
				mono: true,
				missing: "none"
			},
			{
				label: "Specification",
				value: $$props.d.spec,
				mono: true,
				missing: "none attached"
			},
			{
				label: "Tree now",
				value: $$props.d.tree_now ? `${short($$props.d.tree_now.tree) ?? "?"} · ${$$props.d.tree_now.clean ? "clean" : `${$$props.d.tree_now.changed_files ?? 0} uncommitted`}` : null,
				mono: true,
				missing: "unknown"
			},
			{
				label: "Commit",
				value: short($$props.d.tree_now?.commit),
				mono: true,
				missing: "no commits yet"
			},
			{
				label: "Pushed",
				value: $$props.d.tree_now?.reach?.replace(/_/g, " ") ?? null
			},
			{
				label: "Pull request",
				value: $$props.d.pull_request?.url ?? null,
				missing: "not offered"
			},
			{
				label: "Cost",
				value: $$props.d.cost_usd ? `$${$$props.d.cost_usd.toFixed(2)}` : null,
				missing: "not reported"
			},
			{
				label: "Started",
				value: ago($$props.d.created_at)
			},
			{
				label: "Last moved",
				value: ago($$props.d.updated_at)
			}
		]);
		Props(node_16, { get rows() {
			return get($0);
		} });
	}
	reset(section_2);
	var section_3 = sibling(section_2, 2);
	var node_17 = sibling(child(section_3), 2);
	var consequent_13 = ($$anchor) => {
		{
			let $0 = /* @__PURE__ */ user_derived(() => read.data !== null);
			Failed($$anchor, {
				what: "the decisions",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				get stale() {
					return get($0);
				}
			});
		}
	};
	if_block(node_17, ($$render) => {
		if (read.failure) $$render(consequent_13);
	});
	Timeline(sibling(node_17, 2), {
		get marks() {
			return get(marks);
		},
		lanes: [
			"change",
			"gates",
			"decisions"
		]
	});
	reset(section_3);
	var node_19 = sibling(section_3, 2);
	var consequent_14 = ($$anchor) => {
		var section_4 = root_18$1();
		var h2 = child(section_4);
		var text_20 = only_child(sibling(child(h2)), true);
		reset(h2);
		each(sibling(h2, 2), 17, () => $$props.d.reports ?? [], (r) => r.id, ($$anchor, r) => {
			var div_2 = root_17$3();
			var span_22 = child(div_2);
			var b_6 = sibling(child(span_22));
			var text_21 = only_child(b_6, true);
			var text_22 = sibling(b_6);
			reset(span_22);
			var span_23 = sibling(span_22, 2);
			var text_23 = only_child(span_23, true);
			var text_24 = only_child(sibling(span_23, 2), true);
			reset(div_2);
			template_effect(() => {
				set_text(text_21, get(r).target_says);
				set_text(text_22, ` — ${get(r).state_says ?? ""}`);
				set_text(text_23, get(r).age_says);
				set_text(text_24, get(r).quoted);
			});
			append($$anchor, div_2);
		});
		reset(section_4);
		template_effect(() => set_text(text_20, $$props.d.reports?.length));
		append($$anchor, section_4);
	};
	if_block(node_19, ($$render) => {
		if (($$props.d.reports ?? []).length > 0) $$render(consequent_14);
	});
	reset(div);
	template_effect(() => {
		set_text(text_15, $$props.d.runs?.length ?? 0);
		set_text(text_16, ` ${$$props.d.runs?.length === 1 ? "run" : "runs"}`);
	});
	delegated("click", button, () => $$props.go("gates"));
	delegated("click", button_1, () => $$props.go("tasks"));
	delegated("click", button_2, () => $$props.go("ledger"));
	delegated("click", button_3, () => $$props.go("agent"));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/Gates.svelte
var root$31 = /* @__PURE__ */ from_html(`<p class="quiet svelte-wqjyvl">No gate has run against this change yet. The gates run when the agent says it has finished, or when you press <b>Run gates</b>.</p>`);
var root_1$31 = /* @__PURE__ */ from_html(`<code class="tree svelte-wqjyvl" title="the tree this ran against"> </code>`);
var root_2$26 = /* @__PURE__ */ from_html(`<li class="svelte-wqjyvl"> </li>`);
var root_3$24 = /* @__PURE__ */ from_html(`<ul class="fails svelte-wqjyvl"></ul>`);
var root_4$23 = /* @__PURE__ */ from_html(`<pre class="out svelte-wqjyvl"> </pre> <span class="bytes svelte-wqjyvl"> </span>`, 1);
var root_5$20 = /* @__PURE__ */ from_html(`<li><div class="row svelte-wqjyvl"><!> <code class="cmd svelte-wqjyvl"> </code> <span class="exit svelte-wqjyvl"> </span> <span class="dur svelte-wqjyvl"> </span></div> <!> <!></li>`);
var root_6$19 = /* @__PURE__ */ from_html(`<ol class="cmds svelte-wqjyvl"></ol>`);
var root_7$17 = /* @__PURE__ */ from_html(`<section><button class="head svelte-wqjyvl"><!> <!> <b> </b> <span> </span> <span class="verdict svelte-wqjyvl"> </span> <span class="meta svelte-wqjyvl"> </span> <!></button> <!></section>`);
var root_8$17 = /* @__PURE__ */ from_html(`<button class="svelte-wqjyvl"><!> Copy as markdown</button>`);
var root_9$14 = /* @__PURE__ */ from_html(`<p class="said svelte-wqjyvl"> </p>`);
var root_10$14 = /* @__PURE__ */ from_html(`<p class="quiet svelte-wqjyvl">Reading…</p>`);
var root_11$11 = /* @__PURE__ */ from_html(`<pre class="svelte-wqjyvl"> </pre>`);
var root_12$9 = /* @__PURE__ */ from_html(`<h3 class="svelte-wqjyvl"><!></h3>`);
var root_13$9 = /* @__PURE__ */ from_html(`<p class="li svelte-wqjyvl">• <!></p>`);
var root_14$9 = /* @__PURE__ */ from_html(`<p class="row svelte-wqjyvl"><!></p>`);
var root_15$7 = /* @__PURE__ */ from_html(`<p class="svelte-wqjyvl"><!></p>`);
var root_16$4 = /* @__PURE__ */ from_html(`<div class="md svelte-wqjyvl"></div>`);
var root_17$2 = /* @__PURE__ */ from_html(`<div class="gates svelte-wqjyvl"><!> <section class="cert svelte-wqjyvl"><header class="svelte-wqjyvl"><h2 class="svelte-wqjyvl"><!> Certificate</h2> <!></header> <!> <!></section></div>`);
function Gates($$anchor, $$props) {
	push($$props, true);
	let certificate = prop($$props, "certificate", 3, null);
	const attempts = /* @__PURE__ */ user_derived(() => [...$$props.d.gates ?? []].reverse());
	let open = /* @__PURE__ */ state(proxy({}));
	const passed = (g) => g.commands.every((c) => c.outcome.outcome === "exited" && c.outcome.code === 0);
	const key = /* @__PURE__ */ user_derived(() => $$props.id);
	const read = resource(() => get(key) ? `/api/changes/${encodeURIComponent(get(key))}/certificate` : null, { tell: () => `devplane change export ${get(key)}` });
	const cert = /* @__PURE__ */ user_derived(() => read.data ?? certificate());
	let said = /* @__PURE__ */ state("");
	const newest = /* @__PURE__ */ user_derived(() => $$props.d.gates?.[$$props.d.gates.length - 1]?.at ?? "");
	let seenNewest = null;
	user_effect(() => {
		const n = get(newest);
		untrack(() => {
			if (seenNewest !== null && n !== seenNewest) read.reload();
			seenNewest = n;
		});
	});
	async function copy() {
		if (!get(cert)?.markdown) return;
		set(said, await copyText(get(cert).markdown) ? "Copied — paste it into the pull request." : `Nothing was copied — this page has no clipboard. Run: devplane change export ${get(key)}`, true);
	}
	var div = root_17$2();
	var node = child(div);
	var consequent = ($$anchor) => {
		append($$anchor, root$31());
	};
	var alternate = ($$anchor) => {
		var fragment = comment();
		each(first_child(fragment), 19, () => get(attempts), (g) => g.at, ($$anchor, g, i) => {
			const ok = /* @__PURE__ */ user_derived(() => passed(get(g)));
			const shown = /* @__PURE__ */ user_derived(() => get(open)[get(i)] ?? get(i) === 0);
			var section = root_7$17();
			let classes;
			var button = child(section);
			var node_2 = child(button);
			{
				let $0 = /* @__PURE__ */ user_derived(() => get(shown) ? "down" : "right");
				Icon(node_2, {
					get name() {
						return get($0);
					},
					size: 12
				});
			}
			var node_3 = sibling(node_2, 2);
			{
				let $0 = /* @__PURE__ */ user_derived(() => get(ok) ? "check" : "x");
				Icon(node_3, {
					get name() {
						return get($0);
					},
					size: 15
				});
			}
			var b_1 = sibling(node_3, 2);
			var text = only_child(b_1, true);
			var span = sibling(b_1, 2);
			var text_1 = only_child(span);
			var span_1 = sibling(span, 2);
			var text_2 = only_child(span_1, true);
			var span_2 = sibling(span_1, 2);
			var text_3 = only_child(span_2);
			var node_4 = sibling(span_2, 2);
			var consequent_1 = ($$anchor) => {
				var code = root_1$31();
				var text_4 = only_child(code);
				template_effect(($0) => set_text(text_4, `tree ${$0 ?? ""}`), [() => get(g).commit.tree.slice(0, 10)]);
				append($$anchor, code);
			};
			if_block(node_4, ($$render) => {
				if (get(g).commit?.tree) $$render(consequent_1);
			});
			reset(button);
			var node_5 = sibling(button, 2);
			var consequent_4 = ($$anchor) => {
				var ol = root_6$19();
				each(ol, 21, () => get(g).commands, index, ($$anchor, c) => {
					const cok = /* @__PURE__ */ user_derived(() => get(c).outcome.outcome === "exited" && get(c).outcome.code === 0);
					var li = root_5$20();
					let classes_1;
					var div_1 = child(li);
					var node_6 = child(div_1);
					{
						let $0 = /* @__PURE__ */ user_derived(() => get(cok) ? "check" : "x");
						Icon(node_6, {
							get name() {
								return get($0);
							},
							size: 13
						});
					}
					var code_1 = sibling(node_6, 2);
					var text_5 = only_child(code_1, true);
					var span_3 = sibling(code_1, 2);
					var text_6 = only_child(span_3, true);
					var text_7 = only_child(sibling(span_3, 2), true);
					reset(div_1);
					var node_7 = sibling(div_1, 2);
					var consequent_2 = ($$anchor) => {
						var ul = root_3$24();
						each(ul, 20, () => get(c).failures, (f) => f, ($$anchor, f) => {
							var li_1 = root_2$26();
							var text_8 = only_child(li_1, true);
							template_effect(() => set_text(text_8, f));
							append($$anchor, li_1);
						});
						reset(ul);
						append($$anchor, ul);
					};
					if_block(node_7, ($$render) => {
						if (get(c).failures?.length) $$render(consequent_2);
					});
					var node_8 = sibling(node_7, 2);
					var consequent_3 = ($$anchor) => {
						var fragment_1 = root_4$23();
						var pre = first_child(fragment_1);
						var text_9 = only_child(pre, true);
						var text_10 = only_child(sibling(pre, 2));
						template_effect(() => {
							set_text(text_9, get(c).output_tail);
							set_text(text_10, `${get(c).output_bytes ?? "?" ?? ""} bytes · digest ${get(c).output_digest ?? "—" ?? ""}`);
						});
						append($$anchor, fragment_1);
					};
					if_block(node_8, ($$render) => {
						if (get(c).output_tail) $$render(consequent_3);
					});
					reset(li);
					template_effect(($0, $1) => {
						classes_1 = set_class(li, 1, "svelte-wqjyvl", null, classes_1, { cbad: !get(cok) });
						set_text(text_5, get(c).command);
						set_text(text_6, $0);
						set_text(text_7, $1);
					}, [() => exit(get(c).outcome), () => took(get(c).duration_ms)]);
					append($$anchor, li);
				});
				reset(ol);
				append($$anchor, ol);
			};
			if_block(node_5, ($$render) => {
				if (get(shown)) $$render(consequent_4);
			});
			reset(section);
			template_effect(($0, $1) => {
				classes = set_class(section, 1, "attempt svelte-wqjyvl", null, classes, {
					ok: get(ok),
					bad: !get(ok)
				});
				set_attribute(button, "aria-expanded", get(shown));
				set_text(text, get(g).gate);
				set_text(text_1, `attempt ${get(g).attempt ?? ""}`);
				set_text(text_2, get(ok) ? "passed" : "failed");
				set_text(text_3, `${get(g).commands.length ?? ""} commands · ${$0 ?? ""} · ${$1 ?? ""}`);
			}, [() => took(get(g).duration_ms), () => ago(get(g).at)]);
			delegated("click", button, () => set(open, {
				...get(open),
				[get(i)]: !get(shown)
			}, true));
			append($$anchor, section);
		});
		append($$anchor, fragment);
	};
	if_block(node, ($$render) => {
		if (get(attempts).length === 0) $$render(consequent);
		else $$render(alternate, -1);
	});
	var section_1 = sibling(node, 2);
	var header = child(section_1);
	var h2 = child(header);
	Icon(child(h2), {
		name: "shield",
		size: 14
	});
	next();
	reset(h2);
	var node_10 = sibling(h2, 2);
	var consequent_5 = ($$anchor) => {
		var button_1 = root_8$17();
		Icon(child(button_1), {
			name: "file",
			size: 13
		});
		next();
		reset(button_1);
		delegated("click", button_1, copy);
		append($$anchor, button_1);
	};
	if_block(node_10, ($$render) => {
		if (get(cert)?.markdown) $$render(consequent_5);
	});
	reset(header);
	var node_12 = sibling(header, 2);
	var consequent_6 = ($$anchor) => {
		var p_1 = root_9$14();
		var text_11 = only_child(p_1, true);
		template_effect(() => set_text(text_11, get(said)));
		append($$anchor, p_1);
	};
	if_block(node_12, ($$render) => {
		if (get(said)) $$render(consequent_6);
	});
	var node_13 = sibling(node_12, 2);
	var consequent_7 = ($$anchor) => {
		Failed($$anchor, {
			what: "the certificate",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_8 = ($$anchor) => {
		append($$anchor, root_10$14());
	};
	var alternate_2 = ($$anchor) => {
		var div_2 = root_16$4();
		each(div_2, 21, () => blocks(get(cert).markdown), index, ($$anchor, b) => {
			var fragment_3 = comment();
			var node_14 = first_child(fragment_3);
			var consequent_9 = ($$anchor) => {
				var pre_1 = root_11$11();
				var text_12 = only_child(pre_1, true);
				template_effect(() => set_text(text_12, get(b).text));
				append($$anchor, pre_1);
			};
			var consequent_10 = ($$anchor) => {
				var h3 = root_12$9();
				Inline(child(h3), { get segs() {
					return get(b).segs;
				} });
				reset(h3);
				append($$anchor, h3);
			};
			var consequent_11 = ($$anchor) => {
				var p_3 = root_13$9();
				Inline(sibling(child(p_3)), { get segs() {
					return get(b).segs;
				} });
				reset(p_3);
				append($$anchor, p_3);
			};
			var consequent_12 = ($$anchor) => {
				var p_4 = root_14$9();
				Inline(child(p_4), { get segs() {
					return get(b).segs;
				} });
				reset(p_4);
				append($$anchor, p_4);
			};
			var alternate_1 = ($$anchor) => {
				var p_5 = root_15$7();
				Inline(child(p_5), { get segs() {
					return get(b).segs;
				} });
				reset(p_5);
				append($$anchor, p_5);
			};
			if_block(node_14, ($$render) => {
				if (get(b).kind === "pre") $$render(consequent_9);
				else if (get(b).kind === "h") $$render(consequent_10, 1);
				else if (get(b).kind === "li") $$render(consequent_11, 2);
				else if (get(b).kind === "row") $$render(consequent_12, 3);
				else $$render(alternate_1, -1);
			});
			append($$anchor, fragment_3);
		});
		reset(div_2);
		append($$anchor, div_2);
	};
	if_block(node_13, ($$render) => {
		if (!get(cert) && read.failure) $$render(consequent_7);
		else if (!get(cert)) $$render(consequent_8, 1);
		else $$render(alternate_2, -1);
	});
	reset(section_1);
	reset(div);
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/Ledger.svelte
var root$30 = /* @__PURE__ */ from_html(`<button> <span class="svelte-6p9ro4"> </span></button>`);
var root_1$30 = /* @__PURE__ */ from_html(`<p class="quiet svelte-6p9ro4"> <code> </code> has every one.</p>`);
var root_2$25 = /* @__PURE__ */ from_html(`<span> </span>`);
var root_3$23 = /* @__PURE__ */ from_html(`<span class="why svelte-6p9ro4"> </span>`);
var root_4$22 = /* @__PURE__ */ from_html(`<p class="quiet svelte-6p9ro4"> </p>`);
var root_5$19 = /* @__PURE__ */ from_html(`<p> </p>`);
var root_6$18 = /* @__PURE__ */ from_html(`<aside class="detail svelte-6p9ro4"><b> </b> <pre class="svelte-6p9ro4"> </pre> <!></aside>`);
var root_7$16 = /* @__PURE__ */ from_html(`<div class="frame svelte-6p9ro4"><!></div> <!>`, 1);
var root_8$16 = /* @__PURE__ */ from_html(`<div class="ledger svelte-6p9ro4"><div class="chips svelte-6p9ro4" role="group" aria-label="by authority"><button>everything <span class="svelte-6p9ro4"> </span></button> <!></div> <!> <!></div>`);
function Ledger($$anchor, $$props) {
	push($$props, true);
	const LIMIT = 500;
	const key = /* @__PURE__ */ user_derived(() => $$props.id);
	const read = resource(() => get(key) ? `/api/decisions?about=${encodeURIComponent(get(key))}&limit=${LIMIT}` : null, { tell: () => `devplane audit ${get(key)}` });
	const rows = /* @__PURE__ */ user_derived(() => Array.isArray(read.data) ? read.data : null);
	let only = /* @__PURE__ */ state(null);
	let selected = /* @__PURE__ */ state(null);
	const AUTHORITIES = [
		"person",
		"rule",
		"timer",
		"nobody",
		"devplane"
	];
	const counts = /* @__PURE__ */ user_derived(() => AUTHORITIES.map((a) => [a, (get(rows) ?? []).filter((r) => r.authority === a).length]));
	const shown = /* @__PURE__ */ user_derived(() => (get(rows) ?? []).filter((r) => !get(only) || r.authority === get(only)));
	const columns = [
		{
			key: "at",
			label: "When",
			width: 110,
			sort: (r) => r.at
		},
		{
			key: "authority",
			label: "Authority",
			width: 120,
			sort: (r) => r.authority
		},
		{
			key: "action",
			label: "Action",
			width: 150,
			sort: (r) => r.action,
			mono: true
		},
		{
			key: "outcome",
			label: "Outcome",
			width: 110,
			sort: (r) => r.outcome
		},
		{
			key: "subject",
			label: "About",
			width: 320,
			mono: true
		},
		{
			key: "reason",
			label: "Why"
		}
	];
	const pick = /* @__PURE__ */ user_derived(() => get(shown).find((r) => r.id === get(selected)) ?? null);
	var div = root_8$16();
	var div_1 = child(div);
	var button = child(div_1);
	let classes;
	var text$4 = only_child(sibling(child(button)), true);
	reset(button);
	each(sibling(button, 2), 17, () => get(counts), ([a, n]) => a, ($$anchor, $$item) => {
		var $$array = /* @__PURE__ */ user_derived(() => to_array(get($$item), 2));
		let a = () => get($$array)[0];
		let n = () => get($$array)[1];
		var button_1 = root$30();
		let classes_1;
		var text_1 = child(button_1);
		var text_2 = only_child(sibling(text_1), true);
		reset(button_1);
		template_effect(() => {
			button_1.disabled = n() === 0;
			classes_1 = set_class(button_1, 1, "svelte-6p9ro4", null, classes_1, { on: get(only) === a() });
			set_text(text_1, `${a() ?? ""} `);
			set_text(text_2, get(rows) ? n() : "");
		});
		delegated("click", button_1, () => set(only, get(only) === a() ? null : a(), true));
		append($$anchor, button_1);
	});
	reset(div_1);
	var node_1 = sibling(div_1, 2);
	var consequent = ($$anchor) => {
		var p = root_1$30();
		var text_3 = child(p);
		text_3.nodeValue = "The newest 500 are shown; ";
		var text_4 = only_child(sibling(text_3));
		next();
		reset(p);
		template_effect(() => set_text(text_4, `devplane audit ${get(key) ?? ""}`));
		append($$anchor, p);
	};
	if_block(node_1, ($$render) => {
		if (get(rows) && get(rows).length >= LIMIT) $$render(consequent);
	});
	var node_2 = sibling(node_1, 2);
	var consequent_1 = ($$anchor) => {
		{
			let $0 = /* @__PURE__ */ user_derived(() => read.data !== null);
			Failed($$anchor, {
				what: "the decisions",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				get stale() {
					return get($0);
				}
			});
		}
	};
	var alternate_1 = ($$anchor) => {
		var fragment_1 = root_7$16();
		var div_2 = first_child(fragment_1);
		var node_3 = child(div_2);
		{
			const cell = ($$anchor, r = noop, c = noop) => {
				var fragment_2 = comment();
				var node_4 = first_child(fragment_2);
				var consequent_2 = ($$anchor) => {
					var span_2 = root_2$25();
					var text_5 = only_child(span_2, true);
					template_effect(($0) => {
						set_attribute(span_2, "title", r().at);
						set_text(text_5, $0);
					}, [() => ago(r().at)]);
					append($$anchor, span_2);
				};
				var consequent_3 = ($$anchor) => {
					{
						let $0 = /* @__PURE__ */ user_derived(() => r().authority === "person" ? "work" : r().authority === "rule" ? "wait" : r().authority === "devplane" ? "none" : "fail");
						Pill($$anchor, {
							get word() {
								return r().authority;
							},
							get as() {
								return get($0);
							}
						});
					}
				};
				var consequent_4 = ($$anchor) => {
					Pill($$anchor, {
						get word() {
							return r().outcome;
						},
						dot: false
					});
				};
				var consequent_5 = ($$anchor) => {
					var text_6 = text();
					template_effect(() => set_text(text_6, r().action));
					append($$anchor, text_6);
				};
				var consequent_6 = ($$anchor) => {
					var text_7 = text();
					template_effect(() => set_text(text_7, r().subject));
					append($$anchor, text_7);
				};
				var alternate = ($$anchor) => {
					var span_3 = root_3$23();
					var text_8 = only_child(span_3, true);
					template_effect(() => set_text(text_8, r().reason ?? ""));
					append($$anchor, span_3);
				};
				if_block(node_4, ($$render) => {
					if (c().key === "at") $$render(consequent_2);
					else if (c().key === "authority") $$render(consequent_3, 1);
					else if (c().key === "outcome") $$render(consequent_4, 2);
					else if (c().key === "action") $$render(consequent_5, 3);
					else if (c().key === "subject") $$render(consequent_6, 4);
					else $$render(alternate, -1);
				});
				append($$anchor, fragment_2);
			};
			const empty = ($$anchor) => {
				var p_1 = root_4$22();
				var text_9 = only_child(p_1, true);
				template_effect(() => set_text(text_9, get(rows) === null ? "Reading…" : get(only) ? `Nothing about this change was decided by ${get(only)}.` : "Nothing has been decided about this change."));
				append($$anchor, p_1);
			};
			Grid(node_3, {
				id: "change-ledger",
				get columns() {
					return columns;
				},
				get rows() {
					return get(shown);
				},
				key: (r) => r.id,
				label: "decisions about this change",
				get selected() {
					return get(selected);
				},
				set selected($$value) {
					set(selected, $$value, true);
				},
				cell,
				empty,
				$$slots: {
					cell: true,
					empty: true
				}
			});
		}
		reset(div_2);
		var node_5 = sibling(div_2, 2);
		var consequent_8 = ($$anchor) => {
			var aside = root_6$18();
			var b = child(aside);
			var text_10 = only_child(b, true);
			var text_11 = sibling(b);
			var pre = sibling(text_11);
			var text_12 = only_child(pre, true);
			var node_6 = sibling(pre, 2);
			var consequent_7 = ($$anchor) => {
				var p_2 = root_5$19();
				var text_13 = only_child(p_2, true);
				template_effect(() => set_text(text_13, get(pick).reason));
				append($$anchor, p_2);
			};
			if_block(node_6, ($$render) => {
				if (get(pick).reason) $$render(consequent_7);
			});
			reset(aside);
			template_effect(($0) => {
				set_text(text_10, get(pick).action);
				set_text(text_11, ` · ${get(pick).outcome ?? ""} · by ${get(pick).authority ?? ""} · ${$0 ?? ""} `);
				set_text(text_12, get(pick).subject);
			}, [() => new Date(get(pick).at).toLocaleString()]);
			append($$anchor, aside);
		};
		if_block(node_5, ($$render) => {
			if (get(pick)) $$render(consequent_8);
		});
		append($$anchor, fragment_1);
	};
	if_block(node_2, ($$render) => {
		if (read.failure) $$render(consequent_1);
		else $$render(alternate_1, -1);
	});
	reset(div);
	template_effect(() => {
		classes = set_class(button, 1, "svelte-6p9ro4", null, classes, { on: get(only) === null });
		set_text(text$4, get(rows) ? get(rows).length : "");
	});
	delegated("click", button, () => set(only, null));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/Agent.svelte
var root$29 = /* @__PURE__ */ from_html(`<p class="quiet svelte-1w0chr6">No agent has run on this change yet.</p>`);
var root_1$29 = /* @__PURE__ */ from_html(`<span class="quiet svelte-1w0chr6"> </span>`);
var root_2$24 = /* @__PURE__ */ from_html(`<div class="fail svelte-1w0chr6"><!></div>`);
var root_3$22 = /* @__PURE__ */ from_html(`<li class="quiet svelte-1w0chr6">reading…</li>`);
var root_4$21 = /* @__PURE__ */ from_html(`<li class="quiet svelte-1w0chr6">Nothing has been said yet.</li>`);
var root_5$18 = /* @__PURE__ */ from_html(`<li class="quiet svelte-1w0chr6"> <code class="svelte-1w0chr6"> </code> has every one.</li>`);
var root_6$17 = /* @__PURE__ */ from_html(`<li><span class="who svelte-1w0chr6"> </span> <div class="text svelte-1w0chr6"> </div></li>`);
var root_7$15 = /* @__PURE__ */ from_html(`<span class="warn svelte-1w0chr6"> </span> <button type="button" class="danger svelte-1w0chr6">Stop the run</button> <button type="button" class="svelte-1w0chr6">Keep it running</button>`, 1);
var root_8$15 = /* @__PURE__ */ from_html(`<button type="button" class="ghost svelte-1w0chr6"><!> Stop…</button>`);
var root_9$13 = /* @__PURE__ */ from_html(`<p class="said svelte-1w0chr6" role="status"> </p>`);
var root_10$13 = /* @__PURE__ */ from_html(`<li class="svelte-1w0chr6"><!><code class="svelte-1w0chr6"> </code></li>`);
var root_11$10 = /* @__PURE__ */ from_html(`<li class="quiet svelte-1w0chr6"> </li>`);
var root_12$8 = /* @__PURE__ */ from_html(`<code class="svelte-1w0chr6"> </code>`);
var root_13$8 = /* @__PURE__ */ from_html(`<li class="svelte-1w0chr6"><!><span> </span><!></li>`);
var root_14$8 = /* @__PURE__ */ from_html(`<div class="agent svelte-1w0chr6"><section class="convo svelte-1w0chr6"><header class="bar svelte-1w0chr6"><!> <b> </b> <!> <!> <code class="quiet svelte-1w0chr6"> </code> <!></header> <!> <!> <ol class="turns svelte-1w0chr6"><!> <!></ol> <form class="composer svelte-1w0chr6"><textarea rows="3" placeholder="Another turn for the agent — it is queued if one is under way" aria-label="another turn for the agent" class="svelte-1w0chr6"></textarea> <div class="send svelte-1w0chr6"><span class="quiet svelte-1w0chr6">⌘↵ to send</span> <!> <button type="submit" class="primary svelte-1w0chr6"><!> Send</button></div></form> <!></section> <aside class="record svelte-1w0chr6"><h2 class="svelte-1w0chr6">Files written <span class="svelte-1w0chr6"> </span></h2> <ul class="svelte-1w0chr6"></ul> <h2 class="svelte-1w0chr6">Running now <span class="svelte-1w0chr6"> </span></h2> <ul class="svelte-1w0chr6"></ul></aside></div>`);
function Agent($$anchor, $$props) {
	push($$props, true);
	function runState(s) {
		if (typeof s === "string") return { word: s.replace(/_/g, " ") };
		if (s && typeof s === "object" && "waiting" in s) {
			const w = s.waiting;
			if (w === "permission") return {
				word: "waiting on you — permission",
				tone: "wait"
			};
			if (w === "question") return {
				word: "waiting on you — question",
				tone: "wait"
			};
			if (w === "idle") return {
				word: "idle — waiting for the next turn",
				tone: "none"
			};
			if (w === "job") return {
				word: "waiting on a command it started",
				tone: "work"
			};
			if (w && typeof w === "object" && "other" in w) return {
				word: `waiting on you — ${String(w.other)}`,
				tone: "wait"
			};
			return {
				word: "waiting on you",
				tone: "wait"
			};
		}
		return s == null ? null : {
			word: "in a state this page does not know",
			tone: "none"
		};
	}
	const LIMIT = 200;
	const EVERY_MS = 2e3;
	const key = /* @__PURE__ */ user_derived(() => $$props.run);
	const runRead = resource(() => get(key) ? `/api/runs/${encodeURIComponent(get(key))}` : null, {
		every: EVERY_MS,
		tell: () => `devplane show ${get(key)}`
	});
	const said_ = resource(() => get(key) ? `/api/runs/${encodeURIComponent(get(key))}/messages?limit=${LIMIT}` : null, {
		every: EVERY_MS,
		tell: () => `devplane show ${get(key)}`
	});
	const detail = /* @__PURE__ */ user_derived(() => runRead.data);
	const messages = /* @__PURE__ */ user_derived(() => Array.isArray(said_.data) ? said_.data : []);
	let said = /* @__PURE__ */ state("");
	let draft = /* @__PURE__ */ state("");
	let sending = /* @__PURE__ */ state(false);
	let survives = /* @__PURE__ */ state("");
	user_effect(() => {
		get(key);
		set(survives, "");
		set(said, "");
	});
	const shownState = /* @__PURE__ */ user_derived(() => runState(get(detail)?.state));
	const running = /* @__PURE__ */ user_derived(() => (get(detail)?.recent_tools ?? []).filter((t) => t.ok == null).map((t) => ({
		tool: t.tool,
		what: t.input?.command ?? t.input?.file_path ?? ""
	})));
	async function send() {
		const text = get(draft).trim();
		if (!text || !$$props.run || get(sending)) return;
		set(sending, true);
		try {
			const r = await api(`/api/runs/${encodeURIComponent($$props.run)}/prompt`, {
				method: "POST",
				body: JSON.stringify({ text })
			});
			set(draft, "");
			set(said, r?.says ?? "Sent.", true);
			said_.reload();
		} catch (e) {
			set(said, `That did not land: ${failure(e).says}`);
		} finally {
			set(sending, false);
		}
	}
	async function askStop() {
		try {
			const r = await api(`/api/runs/${encodeURIComponent($$props.run)}/stop`);
			set(survives, r?.says ?? "Stopping ends this run.", true);
		} catch (e) {
			set(said, `That did not land: ${failure(e).says}`);
		}
	}
	async function stop() {
		try {
			await api(`/api/runs/${encodeURIComponent($$props.run)}/stop`, { method: "POST" });
			set(survives, "");
			set(said, "Stopped.");
			runRead.reload();
		} catch (e) {
			set(said, `That did not land: ${failure(e).says}`);
		}
	}
	function keydown(e) {
		if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			send();
		}
	}
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		append($$anchor, root$29());
	};
	var alternate_1 = ($$anchor) => {
		var div = root_14$8();
		var section = child(div);
		var header = child(section);
		var node_1 = child(header);
		Icon(node_1, {
			name: "agent",
			size: 15
		});
		var b = sibling(node_1, 2);
		var text_1 = only_child(b, true);
		var node_2 = sibling(b, 2);
		var consequent_1 = ($$anchor) => {
			var span = root_1$29();
			var text_2 = only_child(span, true);
			template_effect(() => set_text(text_2, get(detail).model));
			append($$anchor, span);
		};
		if_block(node_2, ($$render) => {
			if (get(detail)?.model) $$render(consequent_1);
		});
		var node_3 = sibling(node_2, 2);
		var consequent_2 = ($$anchor) => {
			Pill($$anchor, {
				get word() {
					return get(shownState).word;
				},
				get as() {
					return get(shownState).tone;
				}
			});
		};
		if_block(node_3, ($$render) => {
			if (get(shownState)) $$render(consequent_2);
		});
		var code = sibling(node_3, 2);
		var text_3 = only_child(code, true);
		var node_4 = sibling(code, 2);
		var consequent_3 = ($$anchor) => {
			var span_1 = root_1$29();
			var text_4 = only_child(span_1);
			template_effect(() => set_text(text_4, `· latest of ${$$props.d.runs.length ?? ""} runs`));
			append($$anchor, span_1);
		};
		if_block(node_4, ($$render) => {
			if ($$props.d.runs && $$props.d.runs.length > 1) $$render(consequent_3);
		});
		reset(header);
		var node_5 = sibling(header, 2);
		var consequent_4 = ($$anchor) => {
			var div_1 = root_2$24();
			var node_6 = child(div_1);
			{
				let $0 = /* @__PURE__ */ user_derived(() => said_.data !== null);
				Failed(node_6, {
					what: "the conversation",
					get failure() {
						return said_.failure;
					},
					get at() {
						return said_.at;
					},
					get stale() {
						return get($0);
					}
				});
			}
			reset(div_1);
			append($$anchor, div_1);
		};
		if_block(node_5, ($$render) => {
			if (said_.failure) $$render(consequent_4);
		});
		var node_7 = sibling(node_5, 2);
		var consequent_5 = ($$anchor) => {
			var div_2 = root_2$24();
			var node_8 = child(div_2);
			{
				let $0 = /* @__PURE__ */ user_derived(() => runRead.data !== null);
				Failed(node_8, {
					what: "the run",
					get failure() {
						return runRead.failure;
					},
					get at() {
						return runRead.at;
					},
					get stale() {
						return get($0);
					}
				});
			}
			reset(div_2);
			append($$anchor, div_2);
		};
		if_block(node_7, ($$render) => {
			if (runRead.failure) $$render(consequent_5);
		});
		var ol = sibling(node_7, 2);
		var node_9 = child(ol);
		var consequent_6 = ($$anchor) => {
			append($$anchor, root_3$22());
		};
		var consequent_7 = ($$anchor) => {
			append($$anchor, root_4$21());
		};
		var consequent_8 = ($$anchor) => {
			var li_2 = root_5$18();
			var text_5 = child(li_2);
			text_5.nodeValue = "Only 200 turns are shown here; ";
			var text_6 = only_child(sibling(text_5));
			next();
			reset(li_2);
			template_effect(() => set_text(text_6, `devplane show ${$$props.run ?? ""}`));
			append($$anchor, li_2);
		};
		if_block(node_9, ($$render) => {
			if (said_.phase === "loading") $$render(consequent_6);
			else if (said_.data !== null && get(messages).length === 0) $$render(consequent_7, 1);
			else if (get(messages).length >= LIMIT) $$render(consequent_8, 2);
		});
		each(sibling(node_9, 2), 17, () => get(messages), (m) => m.id, ($$anchor, m) => {
			var li_3 = root_6$17();
			var span_2 = child(li_3);
			var text_7 = only_child(span_2, true);
			var text_8 = only_child(sibling(span_2, 2), true);
			reset(li_3);
			template_effect(() => {
				set_class(li_3, 1, `turn ${get(m).role ?? ""}`, "svelte-1w0chr6");
				set_text(text_7, get(m).role === "user" ? "you" : get(m).role);
				set_text(text_8, get(m).text);
			});
			append($$anchor, li_3);
		});
		reset(ol);
		var form = sibling(ol, 2);
		var textarea = child(form);
		remove_textarea_child(textarea);
		var div_4 = sibling(textarea, 2);
		var node_11 = sibling(child(div_4), 2);
		var consequent_9 = ($$anchor) => {
			var fragment_2 = root_7$15();
			var span_3 = first_child(fragment_2);
			var text_9 = only_child(span_3, true);
			var button = sibling(span_3, 2);
			var button_1 = sibling(button, 2);
			template_effect(() => set_text(text_9, get(survives)));
			delegated("click", button, stop);
			delegated("click", button_1, () => set(survives, ""));
			append($$anchor, fragment_2);
		};
		var alternate = ($$anchor) => {
			var button_2 = root_8$15();
			Icon(child(button_2), {
				name: "stop",
				size: 12
			});
			next();
			reset(button_2);
			delegated("click", button_2, askStop);
			append($$anchor, button_2);
		};
		if_block(node_11, ($$render) => {
			if (get(survives)) $$render(consequent_9);
			else $$render(alternate, -1);
		});
		var button_3 = sibling(node_11, 2);
		Icon(child(button_3), {
			name: "play",
			size: 12
		});
		next();
		reset(button_3);
		reset(div_4);
		reset(form);
		var node_14 = sibling(form, 2);
		var consequent_10 = ($$anchor) => {
			var p_1 = root_9$13();
			var text_10 = only_child(p_1, true);
			template_effect(() => set_text(text_10, get(said)));
			append($$anchor, p_1);
		};
		if_block(node_14, ($$render) => {
			if (get(said)) $$render(consequent_10);
		});
		reset(section);
		var aside = sibling(section, 2);
		var h2 = child(aside);
		var text_11 = only_child(sibling(child(h2)), true);
		reset(h2);
		var ul = sibling(h2, 2);
		each(ul, 20, () => get(detail)?.wrote ?? [], (f) => f, ($$anchor, f) => {
			var li_4 = root_10$13();
			var node_15 = child(li_4);
			Icon(node_15, {
				name: "file",
				size: 12
			});
			var text_12 = only_child(sibling(node_15), true);
			reset(li_4);
			template_effect(() => set_text(text_12, f));
			append($$anchor, li_4);
		}, ($$anchor) => {
			var li_5 = root_11$10();
			var text_13 = only_child(li_5, true);
			template_effect(() => set_text(text_13, get(detail) ? "none yet" : runRead.failure ? "not read — see above" : "reading…"));
			append($$anchor, li_5);
		});
		reset(ul);
		var h2_1 = sibling(ul, 2);
		var text_14 = only_child(sibling(child(h2_1)), true);
		reset(h2_1);
		var ul_1 = sibling(h2_1, 2);
		each(ul_1, 21, () => get(running), index, ($$anchor, c) => {
			var li_6 = root_13$8();
			var node_16 = child(li_6);
			Icon(node_16, {
				name: "terminal",
				size: 12
			});
			var span_6 = sibling(node_16);
			var text_15 = only_child(span_6, true);
			var node_17 = sibling(span_6);
			var consequent_11 = ($$anchor) => {
				var code_3 = root_12$8();
				var text_16 = only_child(code_3, true);
				template_effect(() => set_text(text_16, get(c).what));
				append($$anchor, code_3);
			};
			if_block(node_17, ($$render) => {
				if (get(c).what) $$render(consequent_11);
			});
			reset(li_6);
			template_effect(() => set_text(text_15, get(c).tool));
			append($$anchor, li_6);
		}, ($$anchor) => {
			var li_7 = root_11$10();
			var text_17 = only_child(li_7, true);
			template_effect(() => set_text(text_17, get(detail) ? "no tool call in flight" : runRead.failure ? "not read — see above" : "reading…"));
			append($$anchor, li_7);
		});
		reset(ul_1);
		reset(aside);
		reset(div);
		template_effect(($0, $1) => {
			set_text(text_1, get(detail)?.agent ?? "agent");
			set_text(text_3, $0);
			button_3.disabled = $1;
			set_text(text_11, get(detail) ? get(detail).wrote?.length ?? 0 : "");
			set_text(text_14, get(detail) ? get(running).length : "");
		}, [() => $$props.run.slice(0, 18), () => !get(draft).trim() || get(sending)]);
		event("submit", form, (e) => {
			e.preventDefault();
			send();
		});
		delegated("keydown", textarea, keydown);
		bind_value(textarea, () => get(draft), ($$value) => set(draft, $$value));
		append($$anchor, div);
	};
	if_block(node, ($$render) => {
		if (!$$props.run) $$render(consequent);
		else $$render(alternate_1, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["keydown", "click"]);
//#endregion
//#region src/surfaces/change/pair.ts
function names(step, task) {
	const key = fold(task.text);
	return key.length > 0 && fold(step.content).includes(key);
}
function fold(s) {
	return s.toLowerCase().replace(/\s+/g, " ").trim();
}
function pair(tasks, steps) {
	const taken = /* @__PURE__ */ new Set();
	const rows = tasks.map((task) => {
		const at = steps.findIndex((s, i) => !taken.has(i) && names(s, task));
		if (at === -1) return {
			task,
			step: null
		};
		taken.add(at);
		return {
			task,
			step: steps[at]
		};
	});
	steps.forEach((step, i) => {
		if (!taken.has(i)) rows.push({
			task: null,
			step
		});
	});
	return rows;
}
//#endregion
//#region src/surfaces/change/Plan.svelte
var root$28 = /* @__PURE__ */ from_html(`<p class="dim svelte-rag4cy">No tasks were sent and no plan was reported.</p>`);
var root_1$28 = /* @__PURE__ */ from_html(`<span class="where svelte-rag4cy"> </span>`);
var root_2$23 = /* @__PURE__ */ from_html(`<span class="text svelte-rag4cy"> </span><!>`, 1);
var root_3$21 = /* @__PURE__ */ from_html(`<span class="status svelte-rag4cy"> </span><span class="text svelte-rag4cy"> </span>`, 1);
var root_4$20 = /* @__PURE__ */ from_html(`<li><span class="task svelte-rag4cy"><!></span> <span class="step svelte-rag4cy"><!></span></li>`);
var root_5$17 = /* @__PURE__ */ from_html(`<ol class="rows svelte-rag4cy"></ol> <p class="dim tally svelte-rag4cy"> </p>`, 1);
var root_6$16 = /* @__PURE__ */ from_html(`<section class="plan" aria-label="tasks and the agent's plan"><div class="heads svelte-rag4cy"><h3 class="svelte-rag4cy">tasks sent <span class="n svelte-rag4cy"> </span></h3> <h3 class="svelte-rag4cy">the agent's plan <span class="n svelte-rag4cy"> </span></h3></div> <!></section>`);
function Plan($$anchor, $$props) {
	push($$props, true);
	let tasks = prop($$props, "tasks", 19, () => []), steps = prop($$props, "steps", 19, () => []);
	const rows = /* @__PURE__ */ user_derived(() => pair(tasks(), steps()));
	const matched = /* @__PURE__ */ user_derived(() => get(rows).filter((r) => r.task && r.step).length);
	var section = root_6$16();
	var div = child(section);
	var h3 = child(div);
	var text = only_child(sibling(child(h3)), true);
	reset(h3);
	var h3_1 = sibling(h3, 2);
	var text_1 = only_child(sibling(child(h3_1)), true);
	reset(h3_1);
	reset(div);
	var node = sibling(div, 2);
	var consequent = ($$anchor) => {
		append($$anchor, root$28());
	};
	var alternate = ($$anchor) => {
		var fragment = root_5$17();
		var ol = first_child(fragment);
		each(ol, 21, () => get(rows), index, ($$anchor, r) => {
			var li = root_4$20();
			let classes;
			var span_2 = child(li);
			var node_1 = child(span_2);
			var consequent_2 = ($$anchor) => {
				var fragment_1 = root_2$23();
				var span_3 = first_child(fragment_1);
				var text_2 = only_child(span_3, true);
				var node_2 = sibling(span_3);
				var consequent_1 = ($$anchor) => {
					var span_4 = root_1$28();
					var text_3 = only_child(span_4);
					template_effect(() => set_text(text_3, `${get(r).task.path ?? ""}${get(r).task.line ? `:${get(r).task.line}` : ""}`));
					append($$anchor, span_4);
				};
				if_block(node_2, ($$render) => {
					if (get(r).task.path) $$render(consequent_1);
				});
				template_effect(() => set_text(text_2, get(r).task.text));
				append($$anchor, fragment_1);
			};
			if_block(node_1, ($$render) => {
				if (get(r).task) $$render(consequent_2);
			});
			reset(span_2);
			var span_5 = sibling(span_2, 2);
			var node_3 = child(span_5);
			var consequent_3 = ($$anchor) => {
				var fragment_2 = root_3$21();
				var span_6 = first_child(fragment_2);
				var text_4 = only_child(span_6, true);
				var text_5 = only_child(sibling(span_6), true);
				template_effect(($0) => {
					set_text(text_4, $0);
					set_text(text_5, get(r).step.content);
				}, [() => get(r).step.status.replace(/_/g, " ")]);
				append($$anchor, fragment_2);
			};
			if_block(node_3, ($$render) => {
				if (get(r).step) $$render(consequent_3);
			});
			reset(span_5);
			reset(li);
			template_effect(() => classes = set_class(li, 1, "row svelte-rag4cy", null, classes, { pair: !!(get(r).task && get(r).step) }));
			append($$anchor, li);
		});
		reset(ol);
		var text_6 = only_child(sibling(ol, 2));
		template_effect(() => set_text(text_6, `${get(matched) ?? ""} paired · ${tasks().length - get(matched)} tasks alone · ${steps().length - get(matched)} steps alone`));
		append($$anchor, fragment);
	};
	if_block(node, ($$render) => {
		if (tasks().length === 0 && steps().length === 0) $$render(consequent);
		else $$render(alternate, -1);
	});
	reset(section);
	template_effect(() => {
		set_text(text, tasks().length);
		set_text(text_1, steps().length);
	});
	append($$anchor, section);
	pop();
}
//#endregion
//#region src/surfaces/change/Tasks.svelte
var root$27 = /* @__PURE__ */ from_html(`<span class="big quiet svelte-6i9c4j">—</span><span class="lbl svelte-6i9c4j">no gates declared</span>`, 1);
var root_1$27 = /* @__PURE__ */ from_html(`<span class="big svelte-6i9c4j"> </span><span class="lbl svelte-6i9c4j">seen by a passing check</span>`, 1);
var root_2$22 = /* @__PURE__ */ from_html(`<li> </li>`);
var root_3$20 = /* @__PURE__ */ from_html(`<section class="note wait svelte-6i9c4j"><h3 class="svelte-6i9c4j"><!> Ticked, but no run was sent them <span class="n svelte-6i9c4j"> </span></h3> <ul class="svelte-6i9c4j"></ul></section>`);
var root_4$19 = /* @__PURE__ */ from_html(`<section class="note svelte-6i9c4j"><h3 class="svelte-6i9c4j">Sent, not ticked <span class="n svelte-6i9c4j"> </span></h3> <ul class="svelte-6i9c4j"></ul></section>`);
var root_5$16 = /* @__PURE__ */ from_html(`<div class="counts svelte-6i9c4j"><div class="svelte-6i9c4j"><span class="big svelte-6i9c4j"> </span><span class="lbl svelte-6i9c4j">tasks</span></div> <div class="svelte-6i9c4j"><span class="big svelte-6i9c4j"> </span><span class="lbl svelte-6i9c4j">ticked by an agent</span></div> <div class="svelte-6i9c4j"><!></div> <p class="says svelte-6i9c4j"> </p></div> <!> <!>`, 1);
var root_6$15 = /* @__PURE__ */ from_html(`<p class="quiet svelte-6i9c4j"> </p>`);
var root_7$14 = /* @__PURE__ */ from_html(`<div class="drift svelte-6i9c4j"><p class="svelte-6i9c4j"> </p> <button>Tell the run</button> <button>Accept what it saw</button></div>`);
var root_8$14 = /* @__PURE__ */ from_html(`<p class="said svelte-6i9c4j" role="status"> </p>`);
var root_9$12 = /* @__PURE__ */ from_html(`<section class="note wait svelte-6i9c4j"><h3 class="svelte-6i9c4j"><!> The specification changed under a run</h3> <!> <!></section>`);
var root_10$12 = /* @__PURE__ */ from_html(`<tr><td class="mono svelte-6i9c4j"> </td><td class="svelte-6i9c4j"> </td><td class="svelte-6i9c4j"> </td><td> </td><td class="quiet svelte-6i9c4j"> </td></tr>`);
var root_11$9 = /* @__PURE__ */ from_html(`<section><h2 class="svelte-6i9c4j">Requirements and the tasks that cite them</h2> <table class="svelte-6i9c4j"><thead><tr><th class="svelte-6i9c4j">requirement</th><th class="svelte-6i9c4j">tasks</th><th class="svelte-6i9c4j">ticked</th><th class="svelte-6i9c4j">seen by a passing check</th><th class="svelte-6i9c4j"></th></tr></thead><tbody></tbody></table></section>`);
var root_12$7 = /* @__PURE__ */ from_html(`<p class="quiet svelte-6i9c4j" aria-busy="true">Reading the run's own plan…</p>`);
var root_13$7 = /* @__PURE__ */ from_html(`<li><code class="svelte-6i9c4j"> </code> </li>`);
var root_14$7 = /* @__PURE__ */ from_html(`<section><h2 class="svelte-6i9c4j">Every run</h2> <ul class="runs svelte-6i9c4j"></ul></section>`);
var root_15$6 = /* @__PURE__ */ from_html(`<section><h2 class="svelte-6i9c4j"> </h2> <ol class="outline svelte-6i9c4j"></ol></section>`);
var root_16$3 = /* @__PURE__ */ from_html(`<div class="tasks svelte-6i9c4j"><!> <!> <!> <section><h2 class="svelte-6i9c4j">What the latest run was sent, and its own plan</h2> <!> <!></section> <!> <!></div>`);
function Tasks($$anchor, $$props) {
	push($$props, true);
	let reload = prop($$props, "reload", 3, async () => {});
	const run = /* @__PURE__ */ user_derived(() => $$props.lastRun);
	const read = resource(() => get(run) ? `/api/runs/${encodeURIComponent(get(run))}` : null, { tell: () => `devplane show ${get(run)}` });
	const steps = /* @__PURE__ */ user_derived(() => read.data?.plan ?? []);
	const planKnown = /* @__PURE__ */ user_derived(() => !get(run) || read.phase !== "loading");
	let said = /* @__PURE__ */ state("");
	const pending = writer();
	const sent = /* @__PURE__ */ user_derived(() => ($$props.d.run_rows ?? []).find((r) => r.id === $$props.lastRun)?.sent ?? $$props.d.run_rows?.[$$props.d.run_rows.length - 1]?.sent ?? []);
	async function decide(verb, run) {
		set(said, "");
		await pending.run(verb === "tell" ? "Telling the run" : "Accepting", async () => {
			try {
				const id = encodeURIComponent($$props.d.id);
				await api(verb === "tell" ? `/api/changes/${id}/drift/tell` : `/api/changes/${id}/drift/accept`, {
					method: "POST",
					body: JSON.stringify({ run })
				});
				set(said, verb === "tell" ? "Told — the run was handed the files that changed." : "Accepted — the change now works to what the run saw.", true);
				await reload()();
			} catch (e) {
				set(said, `That did not land: ${failure(e).says}`);
			}
		});
	}
	var div = root_16$3();
	var node = child(div);
	var consequent_3 = ($$anchor) => {
		var fragment = root_5$16();
		var div_1 = first_child(fragment);
		var div_2 = child(div_1);
		var text = only_child(child(div_2), true);
		next();
		reset(div_2);
		var div_3 = sibling(div_2, 2);
		var text_1 = only_child(child(div_3), true);
		next();
		reset(div_3);
		var div_4 = sibling(div_3, 2);
		var node_1 = child(div_4);
		var consequent = ($$anchor) => {
			var fragment_1 = root$27();
			next();
			append($$anchor, fragment_1);
		};
		var alternate = ($$anchor) => {
			var fragment_2 = root_1$27();
			var text_2 = only_child(first_child(fragment_2), true);
			next();
			template_effect(() => set_text(text_2, $$props.d.counts.seen_by_pass));
			append($$anchor, fragment_2);
		};
		if_block(node_1, ($$render) => {
			if ($$props.d.counts.seen_by_pass == null) $$render(consequent);
			else $$render(alternate, -1);
		});
		reset(div_4);
		var text_3 = only_child(sibling(div_4, 2), true);
		reset(div_1);
		var node_2 = sibling(div_1, 2);
		var consequent_1 = ($$anchor) => {
			var section = root_3$20();
			var h3 = child(section);
			var node_3 = child(h3);
			Icon(node_3, {
				name: "alert",
				size: 14
			});
			var text_4 = only_child(sibling(node_3, 2), true);
			reset(h3);
			var ul = sibling(h3, 2);
			each(ul, 20, () => $$props.d.counts.ticked_unsent, (t) => t, ($$anchor, t) => {
				var li = root_2$22();
				var text_5 = only_child(li, true);
				template_effect(() => set_text(text_5, t));
				append($$anchor, li);
			});
			reset(ul);
			reset(section);
			template_effect(() => set_text(text_4, $$props.d.counts.ticked_unsent.length));
			append($$anchor, section);
		};
		if_block(node_2, ($$render) => {
			if ($$props.d.counts.ticked_unsent?.length) $$render(consequent_1);
		});
		var node_4 = sibling(node_2, 2);
		var consequent_2 = ($$anchor) => {
			var section_1 = root_4$19();
			var h3_1 = child(section_1);
			var text_6 = only_child(sibling(child(h3_1)), true);
			reset(h3_1);
			var ul_1 = sibling(h3_1, 2);
			each(ul_1, 20, () => $$props.d.counts.sent_unticked, (t) => t, ($$anchor, t) => {
				var li_1 = root_2$22();
				var text_7 = only_child(li_1, true);
				template_effect(() => set_text(text_7, t));
				append($$anchor, li_1);
			});
			reset(ul_1);
			reset(section_1);
			template_effect(() => set_text(text_6, $$props.d.counts.sent_unticked.length));
			append($$anchor, section_1);
		};
		if_block(node_4, ($$render) => {
			if ($$props.d.counts.sent_unticked?.length) $$render(consequent_2);
		});
		template_effect(() => {
			set_text(text, $$props.d.counts.tasks);
			set_text(text_1, $$props.d.counts.ticked);
			set_text(text_3, $$props.d.counts_says);
		});
		append($$anchor, fragment);
	};
	var alternate_1 = ($$anchor) => {
		var p_1 = root_6$15();
		var text_8 = only_child(p_1, true);
		template_effect(() => set_text(text_8, $$props.d.spec ? `${$$props.d.spec} has no task file this change can read.` : "No specification is attached to this change, so there are no tasks to trace."));
		append($$anchor, p_1);
	};
	if_block(node, ($$render) => {
		if ($$props.d.counts) $$render(consequent_3);
		else $$render(alternate_1, -1);
	});
	var node_5 = sibling(node, 2);
	var consequent_6 = ($$anchor) => {
		var section_2 = root_9$12();
		var h3_2 = child(section_2);
		Icon(child(h3_2), {
			name: "alert",
			size: 14
		});
		next();
		reset(h3_2);
		var node_7 = sibling(h3_2, 2);
		each(node_7, 17, () => $$props.d.drifts ?? [], index, ($$anchor, x) => {
			var div_5 = root_7$14();
			var p_2 = child(div_5);
			var text_9 = only_child(p_2, true);
			var button = sibling(p_2, 2);
			var button_1 = sibling(button, 2);
			reset(div_5);
			template_effect(() => {
				set_text(text_9, get(x).says);
				button.disabled = !!pending.busy;
				button_1.disabled = !!pending.busy;
			});
			delegated("click", button, () => decide("tell", get(x).run));
			delegated("click", button_1, () => decide("accept", get(x).run));
			append($$anchor, div_5);
		});
		var node_8 = sibling(node_7, 2);
		var consequent_4 = ($$anchor) => {
			var p_3 = root_8$14();
			var text_10 = only_child(p_3);
			template_effect(() => set_text(text_10, `${pending.busy ?? ""}… ${pending.elapsed ?? ""}`));
			append($$anchor, p_3);
		};
		var consequent_5 = ($$anchor) => {
			var p_4 = root_8$14();
			var text_11 = only_child(p_4, true);
			template_effect(() => set_text(text_11, get(said)));
			append($$anchor, p_4);
		};
		if_block(node_8, ($$render) => {
			if (pending.busy) $$render(consequent_4);
			else if (get(said)) $$render(consequent_5, 1);
		});
		reset(section_2);
		append($$anchor, section_2);
	};
	if_block(node_5, ($$render) => {
		if (($$props.d.drifts ?? []).length > 0) $$render(consequent_6);
	});
	var node_9 = sibling(node_5, 2);
	var consequent_7 = ($$anchor) => {
		var section_3 = root_11$9();
		var table = sibling(child(section_3), 2);
		var tbody = sibling(child(table));
		each(tbody, 21, () => $$props.d.token_rows ?? [], (r) => r.token, ($$anchor, r) => {
			var tr = root_10$12();
			var td = child(tr);
			var text_12 = only_child(td, true);
			var td_1 = sibling(td);
			var text_13 = only_child(td_1, true);
			var td_2 = sibling(td_1);
			var text_14 = only_child(td_2, true);
			var td_3 = sibling(td_2);
			let classes;
			var text_15 = only_child(td_3, true);
			var text_16 = only_child(sibling(td_3), true);
			reset(tr);
			template_effect(() => {
				set_text(text_12, get(r).token);
				set_text(text_13, get(r).tasks);
				set_text(text_14, get(r).ticked);
				classes = set_class(td_3, 1, "svelte-6i9c4j", null, classes, { quiet: get(r).seen_by_pass == null });
				set_text(text_15, get(r).seen_by_pass ?? "no gates");
				set_text(text_16, get(r).says ?? "");
			});
			append($$anchor, tr);
		});
		reset(tbody);
		reset(table);
		reset(section_3);
		append($$anchor, section_3);
	};
	if_block(node_9, ($$render) => {
		if (($$props.d.token_rows ?? []).length > 0) $$render(consequent_7);
	});
	var section_4 = sibling(node_9, 2);
	var node_10 = sibling(child(section_4), 2);
	var consequent_8 = ($$anchor) => {
		{
			let $0 = /* @__PURE__ */ user_derived(() => read.data !== null);
			Failed($$anchor, {
				what: "the run's own plan",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				get stale() {
					return get($0);
				}
			});
		}
	};
	if_block(node_10, ($$render) => {
		if (read.failure) $$render(consequent_8);
	});
	var node_11 = sibling(node_10, 2);
	var consequent_9 = ($$anchor) => {
		{
			let $0 = /* @__PURE__ */ user_derived(() => get(sent).map((t) => ({
				text: t.text,
				path: t.path,
				line: t.line
			})));
			Plan($$anchor, {
				get tasks() {
					return get($0);
				},
				get steps() {
					return get(steps);
				}
			});
		}
	};
	var alternate_2 = ($$anchor) => {
		append($$anchor, root_12$7());
	};
	if_block(node_11, ($$render) => {
		if (get(planKnown)) $$render(consequent_9);
		else $$render(alternate_2, -1);
	});
	reset(section_4);
	var node_12 = sibling(section_4, 2);
	var consequent_10 = ($$anchor) => {
		var section_5 = root_14$7();
		var ul_2 = sibling(child(section_5), 2);
		each(ul_2, 21, () => $$props.d.run_rows ?? [], (r) => r.id, ($$anchor, r) => {
			var li_2 = root_13$7();
			var code = child(li_2);
			var text_17 = only_child(code, true);
			var text_18 = sibling(code);
			reset(li_2);
			template_effect(($0) => {
				set_text(text_17, $0);
				set_text(text_18, ` ${get(r).sent_says ?? ""}`);
			}, [() => get(r).id.slice(0, 16)]);
			append($$anchor, li_2);
		});
		reset(ul_2);
		reset(section_5);
		append($$anchor, section_5);
	};
	if_block(node_12, ($$render) => {
		if (($$props.d.run_rows ?? []).length > 1) $$render(consequent_10);
	});
	var node_13 = sibling(node_12, 2);
	var consequent_11 = ($$anchor) => {
		var section_6 = root_15$6();
		var h2 = child(section_6);
		var text_19 = only_child(h2);
		var ol = sibling(h2, 2);
		each(ol, 21, () => $$props.d.plan.outline, index, ($$anchor, h) => {
			var li_3 = root_2$22();
			let styles;
			var text_20 = only_child(li_3, true);
			template_effect(($0) => {
				set_class(li_3, 1, `h${get(h).level ?? ""}`, "svelte-6i9c4j");
				styles = set_style(li_3, "", styles, { "padding-left": $0 });
				set_text(text_20, get(h).text);
			}, [() => `${Math.max(0, get(h).level - 1) * 1}rem`]);
			append($$anchor, li_3);
		});
		reset(ol);
		reset(section_6);
		template_effect(() => set_text(text_19, `Outline of ${$$props.d.plan.path ?? ""}`));
		append($$anchor, section_6);
	};
	if_block(node_13, ($$render) => {
		if ($$props.d.plan?.outline?.length) $$render(consequent_11);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/lib/ui/Split.svelte
var root$26 = /* @__PURE__ */ from_html(`<div class="pane svelte-l5jdby"><!></div>  <div class="handle svelte-l5jdby" role="separator" tabindex="0" aria-label="resize"></div>`, 1);
var root_1$26 = /* @__PURE__ */ from_html(`<div><!> <div class="rest svelte-l5jdby"><!></div></div>`);
function Split($$anchor, $$props) {
	let axis = prop($$props, "axis", 3, "x"), initial = prop($$props, "size", 3, 280), min = prop($$props, "min", 3, 160), max = prop($$props, "max", 3, 640), side = prop($$props, "side", 3, "start"), collapsed = prop($$props, "collapsed", 3, false);
	const store = () => `vp-split:${$$props.id}`;
	function remembered() {
		try {
			const v = Number(localStorage.getItem(store()));
			return Number.isFinite(v) && v >= min() && v <= max() ? v : initial();
		} catch {
			return initial();
		}
	}
	let size = /* @__PURE__ */ state(proxy(remembered()));
	let dragging = /* @__PURE__ */ state(false);
	function keep() {
		try {
			localStorage.setItem(store(), String(Math.round(get(size))));
		} catch {}
	}
	let start = 0;
	let from = 0;
	function down(e) {
		set(dragging, true);
		start = axis() === "x" ? e.clientX : e.clientY;
		from = get(size);
		e.currentTarget.setPointerCapture(e.pointerId);
	}
	function move(e) {
		if (!get(dragging)) return;
		const delta = (axis() === "x" ? e.clientX : e.clientY) - start;
		set(size, Math.min(max(), Math.max(min(), from + (side() === "start" ? delta : -delta))), true);
	}
	function up() {
		if (!get(dragging)) return;
		set(dragging, false);
		keep();
	}
	function key(e) {
		const i = (axis() === "x" ? ["ArrowRight", "ArrowLeft"] : ["ArrowDown", "ArrowUp"]).indexOf(e.key);
		if (i === -1) return;
		e.preventDefault();
		const step = e.shiftKey ? 48 : 16;
		const sign = (i === 0 ? 1 : -1) * (side() === "start" ? 1 : -1);
		set(size, Math.min(max(), Math.max(min(), get(size) + sign * step)), true);
		keep();
	}
	var div = root_1$26();
	let classes;
	var node = child(div);
	var consequent = ($$anchor) => {
		var fragment = root$26();
		var div_1 = first_child(fragment);
		snippet(child(div_1), () => $$props.pane);
		reset(div_1);
		var div_2 = sibling(div_1, 2);
		template_effect(($0) => {
			set_style(div_1, `${axis() === "x" ? "width" : "height"}: ${get(size) ?? ""}px`);
			set_attribute(div_2, "aria-orientation", axis() === "x" ? "vertical" : "horizontal");
			set_attribute(div_2, "aria-valuenow", $0);
			set_attribute(div_2, "aria-valuemin", min());
			set_attribute(div_2, "aria-valuemax", max());
		}, [() => Math.round(get(size))]);
		delegated("pointerdown", div_2, down);
		delegated("pointermove", div_2, move);
		delegated("pointerup", div_2, up);
		event("pointercancel", div_2, up);
		delegated("keydown", div_2, key);
		append($$anchor, fragment);
	};
	if_block(node, ($$render) => {
		if (!collapsed()) $$render(consequent);
	});
	var div_3 = sibling(node, 2);
	snippet(child(div_3), () => $$props.children);
	reset(div_3);
	reset(div);
	template_effect(() => classes = set_class(div, 1, `split ${axis() ?? ""}`, "svelte-l5jdby", classes, {
		dragging: get(dragging),
		end: side() === "end"
	}));
	append($$anchor, div);
}
delegate([
	"pointerdown",
	"pointermove",
	"pointerup",
	"keydown"
]);
//#endregion
//#region src/surfaces/review/Files.svelte
var root$25 = /* @__PURE__ */ from_html(`<span class="cov svelte-15tjoes"><!></span>`);
var root_1$25 = /* @__PURE__ */ from_html(`<span> </span>`);
var root_2$21 = /* @__PURE__ */ from_html(`<button class="file svelte-15tjoes"><!> <span class="name svelte-15tjoes"><span class="dir svelte-15tjoes"> </span><b class="svelte-15tjoes"> </b></span> <span class="delta svelte-15tjoes"><span class="add svelte-15tjoes"> </span> <span class="del svelte-15tjoes"> </span></span> <span class="meta svelte-15tjoes"><!> <!></span></button>`);
var root_3$19 = /* @__PURE__ */ from_html(`<button><!> <span> </span> <span class="n svelte-15tjoes"> </span></button> <!>`, 1);
var root_4$18 = /* @__PURE__ */ from_html(`<nav class="files svelte-15tjoes" aria-label="files in the change"></nav>`);
function Files($$anchor, $$props) {
	push($$props, true);
	let folded = /* @__PURE__ */ state(proxy({}));
	const split = (p) => {
		const i = p.lastIndexOf("/");
		return i === -1 ? ["", p] : [p.slice(0, i + 1), p.slice(i + 1)];
	};
	const statusIcon = (s) => s.startsWith("added") ? "plus" : s.startsWith("deleted") ? "x" : "file";
	var nav = root_4$18();
	each(nav, 21, () => $$props.sections, (s) => s.says, ($$anchor, s) => {
		var fragment = root_3$19();
		var button = first_child(fragment);
		var node = child(button);
		{
			let $0 = /* @__PURE__ */ user_derived(() => get(folded)[get(s).says] ? "right" : "down");
			Icon(node, {
				get name() {
					return get($0);
				},
				size: 12
			});
		}
		var span = sibling(node, 2);
		var text = only_child(span, true);
		var text_1 = only_child(sibling(span, 2), true);
		reset(button);
		var node_1 = sibling(button, 2);
		var consequent_2 = ($$anchor) => {
			var fragment_1 = comment();
			each(first_child(fragment_1), 17, () => get(s).files, (f) => get(s).says + f.path, ($$anchor, f) => {
				const computed_const = /* @__PURE__ */ user_derived(() => {
					const [dir, base] = split(get(f).path);
					return {
						dir,
						base
					};
				});
				const done = /* @__PURE__ */ user_derived(() => $$props.marked(get(f)));
				var button_1 = root_2$21();
				var node_3 = child(button_1);
				{
					let $0 = /* @__PURE__ */ user_derived(() => statusIcon(get(f).status_says));
					Icon(node_3, {
						get name() {
							return get($0);
						},
						size: 13
					});
				}
				var span_2 = sibling(node_3, 2);
				var span_3 = child(span_2);
				var text_2 = only_child(span_3, true);
				var text_3 = only_child(sibling(span_3), true);
				reset(span_2);
				var span_4 = sibling(span_2, 2);
				var span_5 = child(span_4);
				var text_4 = only_child(span_5);
				var text_5 = only_child(sibling(span_5, 2));
				reset(span_4);
				var span_7 = sibling(span_4, 2);
				var node_4 = child(span_7);
				var consequent = ($$anchor) => {
					var span_8 = root$25();
					Icon(child(span_8), {
						name: "shield",
						size: 11
					});
					reset(span_8);
					template_effect(() => set_attribute(span_8, "title", get(f).coverage_says ?? ""));
					append($$anchor, span_8);
				};
				if_block(node_4, ($$render) => {
					if (get(f).coverage?.coverage === "covered") $$render(consequent);
				});
				var node_6 = sibling(node_4, 2);
				var consequent_1 = ($$anchor) => {
					var span_9 = root_1$25();
					let classes;
					var text_6 = only_child(span_9);
					template_effect(() => {
						classes = set_class(span_9, 1, "seen svelte-15tjoes", null, classes, { all: get(done) === get(f).hunks.length });
						set_attribute(span_9, "title", `${get(done) ?? ""} of ${get(f).hunks.length ?? ""} hunks marked in this browser`);
						set_text(text_6, `${get(done) ?? ""}/${get(f).hunks.length ?? ""}`);
					});
					append($$anchor, span_9);
				};
				if_block(node_6, ($$render) => {
					if (get(f).hunks.length) $$render(consequent_1);
				});
				reset(span_7);
				reset(button_1);
				template_effect(() => {
					set_attribute(button_1, "aria-current", get(f).path === $$props.selected ? "true" : void 0);
					set_attribute(button_1, "title", get(f).path);
					set_text(text_2, get(computed_const).dir);
					set_text(text_3, get(computed_const).base);
					set_text(text_4, `+${get(f).added ?? ""}`);
					set_text(text_5, `−${get(f).removed ?? ""}`);
				});
				delegated("click", button_1, () => $$props.pick(get(f).path));
				append($$anchor, button_1);
			});
			append($$anchor, fragment_1);
		};
		if_block(node_1, ($$render) => {
			if (!get(folded)[get(s).says]) $$render(consequent_2);
		});
		template_effect(() => {
			set_class(button, 1, `sec ${get(s).tone ?? "" ?? ""}`, "svelte-15tjoes");
			set_attribute(button, "aria-expanded", !get(folded)[get(s).says]);
			set_text(text, get(s).says);
			set_text(text_1, get(s).files.length);
		});
		delegated("click", button, () => set(folded, {
			...get(folded),
			[get(s).says]: !get(folded)[get(s).says]
		}, true));
		append($$anchor, fragment);
	});
	reset(nav);
	append($$anchor, nav);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/review/Diff.svelte
var root$24 = /* @__PURE__ */ from_html(`<span title="marked in this browser"><!> marked</span>`);
var root_1$24 = /* @__PURE__ */ from_html(`<button class="folded svelte-16uf83a"><!> </button>`);
var root_2$20 = /* @__PURE__ */ from_html(`<tr><td class="no svelte-16uf83a"> </td><td class="no svelte-16uf83a"> </td><td class="sg svelte-16uf83a"> </td><td class="tx svelte-16uf83a"> </td></tr>`);
var root_3$18 = /* @__PURE__ */ from_html(`<table class="code unified svelte-16uf83a"><tbody></tbody></table>`);
var root_4$17 = /* @__PURE__ */ from_html(`<tr><td> </td><td> </td><td> </td><td> </td><td> </td><td> </td></tr>`);
var root_5$15 = /* @__PURE__ */ from_html(`<table class="code split svelte-16uf83a"><tbody></tbody></table>`);
var root_6$14 = /* @__PURE__ */ from_html(`<section><button class="hh svelte-16uf83a"><code class="svelte-16uf83a"> </code> <!></button> <!></section>`);
function Diff($$anchor, $$props) {
	push($$props, true);
	let mode = prop($$props, "mode", 3, "unified"), current = prop($$props, "current", 19, () => -1), expanded = prop($$props, "expanded", 3, false);
	function numbered(h) {
		const m = /@@ -(\d+)(?:,\d+)? \+(\d+)/.exec(h.header);
		let o = m ? Number(m[1]) : 0;
		let n = m ? Number(m[2]) : 0;
		return h.lines.map(([kind, text]) => {
			if (kind === "added") return {
				kind,
				text,
				old: null,
				new: n++
			};
			if (kind === "removed") return {
				kind,
				text,
				old: o++,
				new: null
			};
			return {
				kind,
				text,
				old: o++,
				new: n++
			};
		});
	}
	function paired(lines) {
		const out = [];
		let i = 0;
		while (i < lines.length) {
			const l = lines[i];
			if (l.kind === "context") {
				out.push({
					left: l,
					right: l
				});
				i++;
				continue;
			}
			const del = [];
			const add = [];
			while (i < lines.length && lines[i].kind === "removed") del.push(lines[i++]);
			while (i < lines.length && lines[i].kind === "added") add.push(lines[i++]);
			for (let k = 0; k < Math.max(del.length, add.length); k++) out.push({
				left: del[k] ?? null,
				right: add[k] ?? null
			});
		}
		return out;
	}
	const sign = (k) => k === "added" ? "+" : k === "removed" ? "−" : " ";
	var fragment = comment();
	each(first_child(fragment), 19, () => $$props.hunks, (h, i) => h.header + i, ($$anchor, h, i) => {
		const m = /* @__PURE__ */ user_derived(() => $$props.mark(get(h)));
		var section = root_6$14();
		let classes;
		var button = child(section);
		var code = child(button);
		var text_1 = only_child(code, true);
		var node_1 = sibling(code, 2);
		var consequent = ($$anchor) => {
			var span = root$24();
			Icon(child(span), {
				name: "eye",
				size: 12
			});
			next();
			reset(span);
			template_effect(() => set_class(span, 1, `mark ${get(m) ?? ""}`, "svelte-16uf83a"));
			append($$anchor, span);
		};
		if_block(node_1, ($$render) => {
			if (get(m)) $$render(consequent);
		});
		reset(button);
		var node_3 = sibling(button, 2);
		var consequent_1 = ($$anchor) => {
			var button_1 = root_1$24();
			var node_4 = child(button_1);
			Icon(node_4, {
				name: "right",
				size: 12
			});
			var text_2 = sibling(node_4);
			reset(button_1);
			template_effect(() => set_text(text_2, ` Formatting only — ${get(h).lines.length ?? ""} lines whose text is unchanged apart from whitespace. Show them.`));
			delegated("click", button_1, function(...$$args) {
				$$props.onexpand?.apply(this, $$args);
			});
			append($$anchor, button_1);
		};
		var consequent_2 = ($$anchor) => {
			var table = root_3$18();
			var tbody = child(table);
			each(tbody, 21, () => numbered(get(h)), index, ($$anchor, l) => {
				var tr = root_2$20();
				var td = child(tr);
				var text_3 = only_child(td, true);
				var td_1 = sibling(td);
				var text_4 = only_child(td_1, true);
				var td_2 = sibling(td_1);
				var text_5 = only_child(td_2, true);
				var text_6 = only_child(sibling(td_2), true);
				reset(tr);
				template_effect(($0) => {
					set_class(tr, 1, clsx(get(l).kind), "svelte-16uf83a");
					set_text(text_3, get(l).old ?? "");
					set_text(text_4, get(l).new ?? "");
					set_text(text_5, $0);
					set_text(text_6, get(l).text);
				}, [() => sign(get(l).kind)]);
				append($$anchor, tr);
			});
			reset(tbody);
			reset(table);
			append($$anchor, table);
		};
		var alternate = ($$anchor) => {
			var table_1 = root_5$15();
			var tbody_1 = child(table_1);
			each(tbody_1, 21, () => paired(numbered(get(h))), index, ($$anchor, p) => {
				var tr_1 = root_4$17();
				var td_4 = child(tr_1);
				var text_7 = only_child(td_4, true);
				var td_5 = sibling(td_4);
				var text_8 = only_child(td_5, true);
				var td_6 = sibling(td_5);
				var text_9 = only_child(td_6, true);
				var td_7 = sibling(td_6);
				var text_10 = only_child(td_7, true);
				var td_8 = sibling(td_7);
				var text_11 = only_child(td_8, true);
				var td_9 = sibling(td_8);
				var text_12 = only_child(td_9, true);
				reset(tr_1);
				template_effect(($0, $1) => {
					set_class(td_4, 1, `no ${get(p).left?.kind ?? "none" ?? ""}`, "svelte-16uf83a");
					set_text(text_7, get(p).left?.old ?? "");
					set_class(td_5, 1, `sg ${get(p).left?.kind ?? "none" ?? ""}`, "svelte-16uf83a");
					set_text(text_8, $0);
					set_class(td_6, 1, `tx ${get(p).left?.kind === "removed" ? "removed" : get(p).left ? "context" : "none"}`, "svelte-16uf83a");
					set_text(text_9, get(p).left?.text ?? "");
					set_class(td_7, 1, `no ${get(p).right?.kind ?? "none" ?? ""}`, "svelte-16uf83a");
					set_text(text_10, get(p).right?.new ?? "");
					set_class(td_8, 1, `sg ${get(p).right?.kind ?? "none" ?? ""}`, "svelte-16uf83a");
					set_text(text_11, $1);
					set_class(td_9, 1, `tx ${get(p).right?.kind === "added" ? "added" : get(p).right ? "context" : "none"}`, "svelte-16uf83a");
					set_text(text_12, get(p).right?.text ?? "");
				}, [() => get(p).left ? sign(get(p).left.kind) : "", () => get(p).right ? sign(get(p).right.kind) : ""]);
				append($$anchor, tr_1);
			});
			reset(tbody_1);
			reset(table_1);
			append($$anchor, table_1);
		};
		if_block(node_3, ($$render) => {
			if (get(h).formatter_only && !expanded()) $$render(consequent_1);
			else if (mode() === "unified") $$render(consequent_2, 1);
			else $$render(alternate, -1);
		});
		reset(section);
		template_effect(() => {
			classes = set_class(section, 1, "hunk svelte-16uf83a", null, classes, { current: get(i) === current() });
			set_attribute(section, "id", `hunk-${get(i) ?? ""}`);
			set_text(text_1, get(h).header);
		});
		delegated("click", button, () => $$props.onpick?.(get(i)));
		append($$anchor, section);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/review/Pane.svelte
var root$23 = /* @__PURE__ */ from_html(`<div class="loading svelte-1dh2e6f">Reading the worktree against its base…</div>`);
var root_1$23 = /* @__PURE__ */ from_html(`<div class="stale svelte-1dh2e6f"><!></div>`);
var root_2$19 = /* @__PURE__ */ from_html(`<p class="finding svelte-1dh2e6f"><!> </p>`);
var root_3$17 = /* @__PURE__ */ from_html(`<div class="tree svelte-1dh2e6f"><!></div>`);
var root_4$16 = /* @__PURE__ */ from_html(`<span class="wrow svelte-1dh2e6f"><!> <code class="matched svelte-1dh2e6f"> </code> <span class="seen svelte-1dh2e6f"> </span></span>`);
var root_5$14 = /* @__PURE__ */ from_html(`<button class="read svelte-1dh2e6f"><!> I have read this</button>`);
var root_6$13 = /* @__PURE__ */ from_html(`<dt class="weak svelte-1dh2e6f">Check weakened</dt> <dd class="weak svelte-1dh2e6f"><!> <!></dd>`, 1);
var root_7$13 = /* @__PURE__ */ from_html(`<dt class="svelte-1dh2e6f">Decided while writing</dt><dd class="svelte-1dh2e6f"> </dd>`, 1);
var root_8$13 = /* @__PURE__ */ from_html(`<code class="cmd svelte-1dh2e6f"> </code>`);
var root_9$11 = /* @__PURE__ */ from_html(`<dt class="svelte-1dh2e6f">Check it yourself</dt><dd class="svelte-1dh2e6f"></dd>`, 1);
var root_10$11 = /* @__PURE__ */ from_html(`<form class="fix svelte-1dh2e6f"><textarea rows="3" aria-label="a message to the latest run" class="svelte-1dh2e6f"></textarea> <div class="svelte-1dh2e6f"><button type="submit" class="primary svelte-1dh2e6f"> </button> <button type="button">Cancel <kbd class="svelte-1dh2e6f">Esc</kbd></button></div></form>`);
var root_11$8 = /* @__PURE__ */ from_html(`<p class="said svelte-1dh2e6f" role="status"> </p>`);
var root_12$6 = /* @__PURE__ */ from_html(`<p class="finding svelte-1dh2e6f"> </p>`);
var root_13$6 = /* @__PURE__ */ from_html(`<header class="fh svelte-1dh2e6f"><div class="path svelte-1dh2e6f"><code class="svelte-1dh2e6f"> </code> <span class="st svelte-1dh2e6f"> </span> <span class="add svelte-1dh2e6f"> </span> <span class="del svelte-1dh2e6f"> </span></div> <dl class="why svelte-1dh2e6f"><!> <dt class="svelte-1dh2e6f">Role</dt><dd class="svelte-1dh2e6f"> </dd> <dt class="svelte-1dh2e6f">Covered by</dt><dd> </dd> <dt class="svelte-1dh2e6f">Asked for</dt><dd> </dd> <!> <!></dl> <div class="acts svelte-1dh2e6f"><button title="marks this hunk in this browser; a weakened line inside it is recorded as read" class="svelte-1dh2e6f"><!> Mark <kbd class="svelte-1dh2e6f">s</kbd></button> <button class="svelte-1dh2e6f"><!> Request a fix <kbd class="svelte-1dh2e6f">f</kbd></button> <span class="hint svelte-1dh2e6f"><kbd class="svelte-1dh2e6f">j</kbd>/<kbd class="svelte-1dh2e6f">k</kbd> hunks · <kbd class="svelte-1dh2e6f">n</kbd>/<kbd class="svelte-1dh2e6f">p</kbd> files</span></div> <!> <!> <!></header> <!> <!>`, 1);
var root_14$6 = /* @__PURE__ */ from_html(`<div class="diff svelte-1dh2e6f"><!></div>`);
var root_15$5 = /* @__PURE__ */ from_html(`<div class="review svelte-1dh2e6f"><header class="bar svelte-1dh2e6f"><span class="base">against <code class="svelte-1dh2e6f"> </code></span> <span class="shape"> </span> <span class="gap svelte-1dh2e6f"></span> <span class="progress svelte-1dh2e6f" title="hunks you have marked in this browser"> </span> <div class="seg svelte-1dh2e6f" role="group" aria-label="order"><button>By risk</button> <button>By intent</button></div> <div class="seg svelte-1dh2e6f" role="group" aria-label="diff layout"><button title="unified"><!></button> <button title="side by side"><!></button></div></header> <!> <!> <div class="body svelte-1dh2e6f"><!></div></div>`);
function Pane($$anchor, $$props) {
	push($$props, true);
	let planted = prop($$props, "review", 3, null), focus = prop($$props, "focus", 3, "");
	const key = /* @__PURE__ */ user_derived(() => $$props.id);
	const read = resource(() => get(key) ? `/api/changes/${encodeURIComponent(get(key))}/review` : null, { tell: () => `devplane change review ${get(key)}` });
	const r = /* @__PURE__ */ user_derived(() => read.data ?? planted());
	function pref(key, dflt) {
		try {
			return localStorage.getItem(key) || dflt;
		} catch {
			return dflt;
		}
	}
	let by = /* @__PURE__ */ state(proxy(pref("vp-review-by", "risk")));
	let mode = /* @__PURE__ */ state(proxy(pref("vp-review-mode", "unified")));
	user_effect(() => {
		try {
			localStorage.setItem("vp-review-by", get(by));
			localStorage.setItem("vp-review-mode", get(mode));
		} catch {}
	});
	const files = /* @__PURE__ */ user_derived(() => (get(r)?.groups ?? []).flatMap((g) => g.files));
	const byPath = /* @__PURE__ */ user_derived(() => new Map(get(files).map((f) => [f.path, f])));
	const sections = /* @__PURE__ */ user_derived(() => {
		if (!get(r)) return [];
		if (get(by) === "risk") return get(r).groups.map((g) => ({
			says: g.says,
			files: g.files,
			tone: g.weakened ? "fail" : void 0
		}));
		const out = get(r).intent.groups.map((g) => ({
			says: `${g.title}`,
			files: g.files.map((p) => get(byPath).get(p)).filter((f) => !!f)
		}));
		if (get(r).intent.not_asked_for.length) out.push({
			says: get(r).intent.not_asked_for_heading,
			files: get(r).intent.not_asked_for.map((n) => get(byPath).get(n.path)).filter((f) => !!f),
			tone: "wait"
		});
		return out;
	});
	const order = /* @__PURE__ */ user_derived(() => get(sections).flatMap((s) => s.files.map((f) => f.path)));
	let picked = /* @__PURE__ */ state("");
	const selected = /* @__PURE__ */ user_derived(() => get(order).includes(get(picked)) ? get(picked) : get(order)[0] ?? "");
	const weakRows = /* @__PURE__ */ user_derived(() => {
		const m = /* @__PURE__ */ new Map();
		for (const w of (get(r)?.groups ?? []).flatMap((g) => g.weakened ?? [])) m.set(w.path, [...m.get(w.path) ?? [], w]);
		return m;
	});
	user_effect(() => {
		if (focus()) set(picked, focus());
	});
	let seenSaid = /* @__PURE__ */ state("");
	async function markRead(rows) {
		const todo = rows.filter((w) => !w.seen);
		if (!todo.length || !$$props.id) return;
		try {
			for (const w of todo) await api(`/api/changes/${encodeURIComponent($$props.id)}/review/seen`, {
				method: "POST",
				body: JSON.stringify({
					path: w.path,
					matched: w.matched
				})
			});
			set(seenSaid, `Marked read, as yours: ${todo.length === 1 ? todo[0].path : `${todo.length} rows`}.`);
			await read.reload();
		} catch (e) {
			set(seenSaid, `That did not land: ${failure(e).says}`);
		}
	}
	const file = /* @__PURE__ */ user_derived(() => get(byPath).get(get(selected)) ?? null);
	let current = /* @__PURE__ */ state(0);
	user_effect(() => {
		get(selected);
		set(current, 0);
	});
	let expanded = /* @__PURE__ */ state(proxy(/* @__PURE__ */ new Set()));
	const keyOf = (f, h) => markKey(get(r)?.change ?? "", f.path, h.header, h.lines);
	let marks = /* @__PURE__ */ state(proxy({}));
	user_effect(() => {
		const next = {};
		const live = /* @__PURE__ */ new Set();
		for (const f of get(files)) for (const h of f.hunks) {
			const k = keyOf(f, h);
			live.add(k);
			const m = readMark(k);
			if (m) next[k] = m;
		}
		if (get(r)?.change && get(files).length && !get(r).truncated_says) prune(get(r).change, live);
		set(marks, next, true);
	});
	const markOf = (f) => (h) => get(marks)[keyOf(f, h)] ?? null;
	const inHunk = (h, w) => w.kind === "skip" && h.lines.some(([k, t]) => k !== "context" && t.trim() === w.matched);
	const marked = (f) => f.hunks.filter((h) => markOf(f)(h)).length;
	const totalHunks = /* @__PURE__ */ user_derived(() => get(files).reduce((n, f) => n + f.hunks.length, 0));
	const totalMarked = /* @__PURE__ */ user_derived(() => Object.keys(get(marks)).length);
	function setMark(m) {
		const h = get(file)?.hunks[get(current)];
		if (!get(file) || !h) return;
		const k = keyOf(get(file), h);
		set(marks, {
			...get(marks),
			[k]: m
		}, true);
		writeMark(k, m);
		markRead((get(weakRows).get(get(file).path) ?? []).filter((w) => inHunk(h, w)));
		move(1);
	}
	function reveal() {
		queueMicrotask(() => document.getElementById(`hunk-${get(current)}`)?.scrollIntoView({
			block: "nearest",
			behavior: "smooth"
		}));
	}
	function move(step) {
		if (!get(file)) return;
		const next = get(current) + step;
		if (next >= 0 && next < get(file).hunks.length) {
			set(current, next);
			reveal();
			return;
		}
		const at = get(order).indexOf(get(selected)) + step;
		if (at >= 0 && at < get(order).length) {
			set(picked, get(order)[at], true);
			queueMicrotask(() => {
				set(current, step === 1 ? 0 : Math.max(0, (get(byPath).get(get(order)[at])?.hunks.length ?? 1) - 1), true);
				reveal();
			});
		}
	}
	function moveFile(step) {
		const at = get(order).indexOf(get(selected)) + step;
		if (at >= 0 && at < get(order).length) set(picked, get(order)[at], true);
	}
	let fixing = /* @__PURE__ */ state(false);
	let draft = /* @__PURE__ */ state("");
	let said = /* @__PURE__ */ state("");
	const sending = writer();
	function requestFix() {
		const h = get(file)?.hunks[get(current)];
		if (!get(file) || !h) return;
		set(draft, `In \`${get(file).path}\` ${h.header}:\n`);
		set(fixing, true);
	}
	async function sendFix() {
		const run = get(r)?.latest_run;
		if (!run || !get(draft).trim()) return;
		await sending.run("Sending to the agent", async () => {
			try {
				const res = await api(`/api/runs/${encodeURIComponent(run)}/prompt`, {
					method: "POST",
					body: JSON.stringify({ text: get(draft).trim() })
				});
				set(said, res.says ?? "Sent to the agent.", true);
				set(fixing, false);
				await read.reload();
			} catch (e) {
				set(said, `That did not land: ${failure(e).says}`);
			}
		});
	}
	function fixKey(e) {
		if (e.key === "Escape") {
			e.preventDefault();
			e.stopPropagation();
			set(fixing, false);
		} else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			sendFix();
		}
	}
	user_effect(() => {
		const offs = [
			onAction("hunk-next", () => (move(1), true)),
			onAction("hunk-prev", () => (move(-1), true)),
			onAction("file-next", () => (moveFile(1), true)),
			onAction("file-prev", () => (moveFile(-1), true)),
			onAction("hunk-seen", () => (setMark("seen"), true)),
			onAction("hunk-fix", () => (requestFix(), true)),
			onAction("hunk-expand", () => (get(file) && set(expanded, /* @__PURE__ */ new Set([...get(expanded), get(file).path]), true), true)),
			onAction("tab-risk", () => (set(by, "risk"), true)),
			onAction("tab-intent", () => (set(by, "intent"), true)),
			onAction("diff-mode", () => (set(mode, get(mode) === "unified" ? "split" : "unified", true), true))
		];
		return () => offs.forEach((off) => off());
	});
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		Failed($$anchor, {
			what: "the review",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_1 = ($$anchor) => {
		append($$anchor, root$23());
	};
	var consequent_2 = ($$anchor) => {
		{
			let $0 = /* @__PURE__ */ user_derived(() => get(r).empty_says ?? "The worktree has no changes against its base.");
			Empty($$anchor, {
				icon: "split",
				title: "Nothing to review yet",
				get body() {
					return get($0);
				}
			});
		}
	};
	var alternate = ($$anchor) => {
		var div_1 = root_15$5();
		var header = child(div_1);
		var span = child(header);
		var text = only_child(sibling(child(span)), true);
		reset(span);
		var span_1 = sibling(span, 2);
		var text_1 = only_child(span_1, true);
		var span_2 = sibling(span_1, 4);
		var text_2 = only_child(span_2);
		var div_2 = sibling(span_2, 2);
		var button = child(div_2);
		let classes;
		var button_1 = sibling(button, 2);
		let classes_1;
		reset(div_2);
		var div_3 = sibling(div_2, 2);
		var button_2 = child(div_3);
		let classes_2;
		Icon(child(button_2), {
			name: "unified",
			size: 14
		});
		reset(button_2);
		var button_3 = sibling(button_2, 2);
		let classes_3;
		Icon(child(button_3), {
			name: "split",
			size: 14
		});
		reset(button_3);
		reset(div_3);
		reset(header);
		var node_3 = sibling(header, 2);
		var consequent_3 = ($$anchor) => {
			var div_4 = root_1$23();
			Failed(child(div_4), {
				what: "the review",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				stale: true
			});
			reset(div_4);
			append($$anchor, div_4);
		};
		if_block(node_3, ($$render) => {
			if (read.phase === "stale" && read.failure) $$render(consequent_3);
		});
		var node_5 = sibling(node_3, 2);
		each(node_5, 17, () => [
			get(r).unordered,
			get(r).coverage_absent,
			get(r).truncated_says,
			get(by) === "intent" ? get(r).intent.unavailable : null
		], index, ($$anchor, s) => {
			var fragment_3 = comment();
			var node_6 = first_child(fragment_3);
			var consequent_4 = ($$anchor) => {
				var p_1 = root_2$19();
				var node_7 = child(p_1);
				Icon(node_7, {
					name: "alert",
					size: 13
				});
				var text_3 = sibling(node_7);
				reset(p_1);
				template_effect(() => set_text(text_3, ` ${get(s) ?? ""}`));
				append($$anchor, p_1);
			};
			if_block(node_6, ($$render) => {
				if (get(s)) $$render(consequent_4);
			});
			append($$anchor, fragment_3);
		});
		var div_5 = sibling(node_5, 2);
		var node_8 = child(div_5);
		{
			const pane = ($$anchor) => {
				var div_6 = root_3$17();
				Files(child(div_6), {
					get sections() {
						return get(sections);
					},
					get selected() {
						return get(selected);
					},
					marked,
					pick: (p) => set(picked, p, true)
				});
				reset(div_6);
				append($$anchor, div_6);
			};
			Split(node_8, {
				id: "review-files",
				size: 300,
				min: 200,
				max: 520,
				pane,
				children: ($$anchor, $$slotProps) => {
					var div_7 = root_14$6();
					var node_10 = child(div_7);
					var consequent_13 = ($$anchor) => {
						var fragment_4 = root_13$6();
						var header_1 = first_child(fragment_4);
						var div_8 = child(header_1);
						var code_1 = child(div_8);
						var text_4 = only_child(code_1, true);
						var span_3 = sibling(code_1, 2);
						var text_5 = only_child(span_3, true);
						var span_4 = sibling(span_3, 2);
						var text_6 = only_child(span_4);
						var text_7 = only_child(sibling(span_4, 2));
						reset(div_8);
						var dl = sibling(div_8, 2);
						var node_11 = child(dl);
						var consequent_6 = ($$anchor) => {
							var fragment_5 = root_6$13();
							var dd = sibling(first_child(fragment_5), 2);
							var node_12 = child(dd);
							each(node_12, 17, () => get(weakRows).get(get(file).path) ?? [], (w) => w.why + w.matched, ($$anchor, w) => {
								var span_6 = root_4$16();
								var node_13 = child(span_6);
								Inline(node_13, { get text() {
									return get(w).why;
								} });
								var code_2 = sibling(node_13, 2);
								var text_8 = only_child(code_2, true);
								var text_9 = only_child(sibling(code_2, 2), true);
								reset(span_6);
								template_effect(() => {
									set_text(text_8, get(w).matched);
									set_text(text_9, get(w).seen ? "marked read" : "not yet read");
								});
								append($$anchor, span_6);
							});
							var node_14 = sibling(node_12, 2);
							var consequent_5 = ($$anchor) => {
								var button_4 = root_5$14();
								Icon(child(button_4), {
									name: "eye",
									size: 13
								});
								next();
								reset(button_4);
								delegated("click", button_4, () => markRead(get(weakRows).get(get(file).path) ?? []));
								append($$anchor, button_4);
							};
							var d = /* @__PURE__ */ user_derived(() => (get(weakRows).get(get(file).path) ?? []).some((w) => !w.seen));
							if_block(node_14, ($$render) => {
								if (get(d)) $$render(consequent_5);
							});
							reset(dd);
							append($$anchor, fragment_5);
						};
						var d_1 = /* @__PURE__ */ user_derived(() => get(weakRows).get(get(file).path));
						if_block(node_11, ($$render) => {
							if (get(d_1)) $$render(consequent_6);
						});
						var dd_1 = sibling(node_11, 3);
						var text_10 = only_child(dd_1, true);
						var dd_2 = sibling(dd_1, 3);
						let classes_4;
						var text_11 = only_child(dd_2, true);
						var dd_3 = sibling(dd_2, 3);
						let classes_5;
						var text_12 = only_child(dd_3, true);
						var node_16 = sibling(dd_3, 2);
						var consequent_7 = ($$anchor) => {
							var fragment_6 = root_7$13();
							var text_13 = only_child(sibling(first_child(fragment_6)), true);
							template_effect(($0) => set_text(text_13, $0), [() => get(file).decisions.map((x) => x.says).join(" · ")]);
							append($$anchor, fragment_6);
						};
						if_block(node_16, ($$render) => {
							if (get(file).decisions.length) $$render(consequent_7);
						});
						var node_17 = sibling(node_16, 2);
						var consequent_8 = ($$anchor) => {
							var fragment_7 = root_9$11();
							var dd_5 = sibling(first_child(fragment_7));
							each(dd_5, 20, () => get(file).marker_commands, (c) => c, ($$anchor, c) => {
								var code_3 = root_8$13();
								var text_14 = only_child(code_3, true);
								template_effect(() => set_text(text_14, c));
								append($$anchor, code_3);
							});
							reset(dd_5);
							append($$anchor, fragment_7);
						};
						if_block(node_17, ($$render) => {
							if (get(file).marker_commands.length) $$render(consequent_8);
						});
						reset(dl);
						var div_9 = sibling(dl, 2);
						var button_5 = child(div_9);
						Icon(child(button_5), {
							name: "eye",
							size: 13
						});
						next(2);
						reset(button_5);
						var button_6 = sibling(button_5, 2);
						Icon(child(button_6), {
							name: "agent",
							size: 13
						});
						next(2);
						reset(button_6);
						next(2);
						reset(div_9);
						var node_20 = sibling(div_9, 2);
						var consequent_9 = ($$anchor) => {
							var form = root_10$11();
							var textarea = child(form);
							remove_textarea_child(textarea);
							autofocus(textarea, true);
							var div_10 = sibling(textarea, 2);
							var button_7 = child(div_10);
							var text_15 = only_child(button_7, true);
							var button_8 = sibling(button_7, 2);
							reset(div_10);
							reset(form);
							template_effect(() => {
								button_7.disabled = !!sending.busy;
								set_text(text_15, sending.busy ? `${sending.busy}… ${sending.elapsed}` : "Send to the agent");
							});
							event("submit", form, (e) => {
								e.preventDefault();
								sendFix();
							});
							delegated("keydown", textarea, fixKey);
							bind_value(textarea, () => get(draft), ($$value) => set(draft, $$value));
							delegated("click", button_8, () => set(fixing, false));
							append($$anchor, form);
						};
						if_block(node_20, ($$render) => {
							if (get(fixing)) $$render(consequent_9);
						});
						var node_21 = sibling(node_20, 2);
						var consequent_10 = ($$anchor) => {
							var p_2 = root_11$8();
							var text_16 = only_child(p_2, true);
							template_effect(() => set_text(text_16, get(said)));
							append($$anchor, p_2);
						};
						if_block(node_21, ($$render) => {
							if (get(said)) $$render(consequent_10);
						});
						var node_22 = sibling(node_21, 2);
						var consequent_11 = ($$anchor) => {
							var p_3 = root_11$8();
							var text_17 = only_child(p_3, true);
							template_effect(() => set_text(text_17, get(seenSaid)));
							append($$anchor, p_3);
						};
						if_block(node_22, ($$render) => {
							if (get(seenSaid)) $$render(consequent_11);
						});
						reset(header_1);
						var node_23 = sibling(header_1, 2);
						var consequent_12 = ($$anchor) => {
							var p_4 = root_12$6();
							var text_18 = only_child(p_4, true);
							template_effect(() => set_text(text_18, get(file).body_says));
							append($$anchor, p_4);
						};
						if_block(node_23, ($$render) => {
							if (get(file).body_says) $$render(consequent_12);
						});
						var node_24 = sibling(node_23, 2);
						{
							let $0 = /* @__PURE__ */ user_derived(() => get(expanded).has(get(file).path));
							let $1 = /* @__PURE__ */ user_derived(() => markOf(get(file)));
							Diff(node_24, {
								get hunks() {
									return get(file).hunks;
								},
								get mode() {
									return get(mode);
								},
								get current() {
									return get(current);
								},
								get expanded() {
									return get($0);
								},
								get mark() {
									return get($1);
								},
								onexpand: () => set(expanded, /* @__PURE__ */ new Set([...get(expanded), get(file).path]), true),
								onpick: (i) => set(current, i, true)
							});
						}
						template_effect(($0) => {
							set_text(text_4, get(file).path);
							set_text(text_5, get(file).status_says);
							set_text(text_6, `+${get(file).added ?? ""}`);
							set_text(text_7, `−${get(file).removed ?? ""}`);
							set_text(text_10, get(file).role_says);
							classes_4 = set_class(dd_2, 1, "svelte-1dh2e6f", null, classes_4, { quiet: get(file).coverage?.coverage !== "covered" });
							set_text(text_11, get(file).coverage_says ?? "no mapping declared");
							classes_5 = set_class(dd_3, 1, "svelte-1dh2e6f", null, classes_5, { wait: $0 });
							set_text(text_12, get(file).task_says);
							button_6.disabled = !get(r).latest_run;
						}, [() => get(file).task_says.startsWith("not asked")]);
						delegated("click", button_5, () => setMark("seen"));
						delegated("click", button_6, requestFix);
						append($$anchor, fragment_4);
					};
					if_block(node_10, ($$render) => {
						if (get(file)) $$render(consequent_13);
					});
					reset(div_7);
					append($$anchor, div_7);
				},
				$$slots: {
					pane: true,
					default: true
				}
			});
		}
		reset(div_5);
		reset(div_1);
		template_effect(() => {
			set_text(text, get(r).base);
			set_text(text_1, get(r).shape_says);
			set_text(text_2, `${get(totalMarked) ?? ""} of ${get(totalHunks) ?? ""} hunks marked`);
			classes = set_class(button, 1, "svelte-1dh2e6f", null, classes, { on: get(by) === "risk" });
			classes_1 = set_class(button_1, 1, "svelte-1dh2e6f", null, classes_1, { on: get(by) === "intent" });
			classes_2 = set_class(button_2, 1, "svelte-1dh2e6f", null, classes_2, { on: get(mode) === "unified" });
			classes_3 = set_class(button_3, 1, "svelte-1dh2e6f", null, classes_3, { on: get(mode) === "split" });
		});
		delegated("click", button, () => set(by, "risk"));
		delegated("click", button_1, () => set(by, "intent"));
		delegated("click", button_2, () => set(mode, "unified"));
		delegated("click", button_3, () => set(mode, "split"));
		append($$anchor, div_1);
	};
	if_block(node, ($$render) => {
		if (read.failure && !get(r)) $$render(consequent);
		else if (!get(r)) $$render(consequent_1, 1);
		else if (get(files).length === 0) $$render(consequent_2, 2);
		else $$render(alternate, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click", "keydown"]);
//#endregion
//#region src/surfaces/change/Doc.svelte
var root$22 = /* @__PURE__ */ from_html(`<div class="failed svelte-178dv07"><!></div>`);
var root_1$22 = /* @__PURE__ */ from_html(`<div class="loading svelte-178dv07"><div class="bar svelte-178dv07"></div><div class="bar short svelte-178dv07"></div></div>`);
var root_2$18 = /* @__PURE__ */ from_html(`<button class="chip mono svelte-178dv07" title="copy the branch name"><!> </button>`);
var root_3$16 = /* @__PURE__ */ from_html(`<span class="chip mono svelte-178dv07"><!> </span>`);
var root_4$15 = /* @__PURE__ */ from_html(`<span class="chip warn svelte-178dv07"><!> in place — no parallel safety</span>`);
var root_5$13 = /* @__PURE__ */ from_html(`<span class="chip svelte-178dv07"> </span>`);
var root_6$12 = /* @__PURE__ */ from_html(`<span class="count svelte-178dv07"> </span>`);
var root_7$12 = /* @__PURE__ */ from_html(`<button class="svelte-178dv07"><!> Run gates</button>`);
var root_8$12 = /* @__PURE__ */ from_html(`<button><!> Offer as pull request</button>`);
var root_9$10 = /* @__PURE__ */ from_html(`<button class="svelte-178dv07"><!> Try again</button>`);
var root_10$10 = /* @__PURE__ */ from_html(`<button class="svelte-178dv07"><!> Pick it back up</button> <!>`, 1);
var root_11$7 = /* @__PURE__ */ from_html(`<button class="svelte-178dv07"><!> Finish</button>`);
var root_12$5 = /* @__PURE__ */ from_html(`<button class="quiet svelte-178dv07" title="open the worktree in your editor"><!> Editor</button> <button class="quiet svelte-178dv07" title="open a terminal in the worktree"><!> Terminal</button>`, 1);
var root_13$5 = /* @__PURE__ */ from_html(`<button class="quiet svelte-178dv07" title="remove the worktree; the branch and the record are kept"><!> Archive</button>`);
var root_14$5 = /* @__PURE__ */ from_html(`<li class="svelte-178dv07"><code class="svelte-178dv07"> </code> <span class="svelte-178dv07"><!></span> <button class="svelte-178dv07"><!> Read it</button></li>`);
var root_15$4 = /* @__PURE__ */ from_html(`<div class="refusal svelte-178dv07" role="alert"><p class="svelte-178dv07"><b>Not offered</b> </p> <ul class="svelte-178dv07"></ul> <p class="how svelte-178dv07">Mark each row seen in the review, or from a terminal of your own: <code class="svelte-178dv07"> </code></p></div>`);
var root_16$2 = /* @__PURE__ */ from_html(`<article class="doc svelte-178dv07"><header class="head svelte-178dv07"><div class="line1 svelte-178dv07"><h1 class="svelte-178dv07"> </h1> <!><!> <!></div> <div class="chips svelte-178dv07"><span class="chip svelte-178dv07"><!> </span> <!> <!> <!> <!></div> <p><!></p> <div class="toolbar svelte-178dv07" role="toolbar" aria-label="what you can do to this change"><button><!> Review<!></button> <!> <!> <!> <!> <span class="sep svelte-178dv07"></span> <!> <!></div> <p role="status" aria-live="polite"><!></p> <!> <!> <!> <!></header> <!> <div class="view svelte-178dv07" role="tabpanel"><!></div></article>`);
function Doc$1($$anchor, $$props) {
	push($$props, true);
	let id = prop($$props, "id", 3, ""), brief = prop($$props, "brief", 3, null), loaded = prop($$props, "loaded", 3, false), planted = prop($$props, "detail", 3, null);
	const key = /* @__PURE__ */ user_derived(id);
	const read = resource(() => get(key) ? `/api/changes/${encodeURIComponent(get(key))}` : null, { tell: () => `devplane change show ${get(key)}` });
	const d = /* @__PURE__ */ user_derived(() => read.data ?? planted());
	let said = /* @__PURE__ */ state("");
	let commands = /* @__PURE__ */ state(proxy([]));
	const pending = writer();
	let view = /* @__PURE__ */ state("overview");
	let refusal = /* @__PURE__ */ state(null);
	let reviewAt = /* @__PURE__ */ state("");
	let lastMoved = null;
	user_pre_effect(() => {
		get(key);
		untrack(() => {
			set(view, "overview");
			set(said, "");
			set(commands, [], true);
			set(refusal, null);
			set(reviewAt, "");
			lastMoved = null;
		});
	});
	const moved = /* @__PURE__ */ user_derived(() => brief()?.state ?? "");
	user_effect(() => {
		const m = get(moved);
		untrack(() => {
			if (lastMoved !== null && m !== lastMoved) read.reload();
			lastMoved = m;
		});
	});
	const verified = /* @__PURE__ */ user_derived(() => get(d)?.state === "verified");
	const unseen = /* @__PURE__ */ user_derived(() => get(d)?.qualifier?.unseen ?? 0);
	function readRow(path) {
		set(reviewAt, path, true);
		set(view, "review");
	}
	const lastRun = /* @__PURE__ */ user_derived(() => get(d)?.runs?.[get(d).runs.length - 1] ?? "");
	const project = /* @__PURE__ */ user_derived(() => (get(d)?.project_id ?? "").split("/").filter(Boolean).pop() ?? "");
	const ROUTE = {
		verify: (id) => `/api/changes/${id}/verify`,
		finish: (id) => `/api/changes/${id}/finish`,
		offer: (id) => `/api/changes/${id}/offer`,
		resume: (id) => `/api/changes/${id}/resume`,
		retry: (id) => `/api/changes/${id}/retry`,
		archive: (id) => `/api/changes/${id}/archive`
	};
	const PAST = {
		verify: "the gates ran",
		finish: "finished — recorded as yours",
		offer: STATES.offered.word,
		resume: "picked back up",
		retry: "trying again",
		archive: "archived — the worktree is removed and the record kept"
	};
	const DOING = {
		verify: "Running the gates",
		finish: "Finishing",
		offer: "Offering",
		resume: "Picking it back up",
		retry: "Trying again",
		archive: "Archiving"
	};
	function outcome(verb, r) {
		if (verb === "offer" && r?.offer !== "opened") return "Nothing was pushed — [github] pull_request is not set, so the pull request is yours to open. Run:";
		if (verb === "verify" && typeof r?.passed === "boolean") return `${r.passed ? "The gates passed" : "The gates failed"}${r.summary ? `: ${r.summary}` : "."}`;
		return (r?.says ?? PAST[verb]) + (r?.pull_request?.url ? ` — ${r.pull_request.url}` : "");
	}
	async function act(verb) {
		const at = get(key);
		if (!at || pending.busy) return;
		await pending.run(verb, async () => {
			try {
				const r = await api(ROUTE[verb](encodeURIComponent(at)), { method: "POST" });
				if (at !== get(key)) return;
				set(said, outcome(verb, r), true);
				set(commands, verb === "offer" && r?.offer !== "opened" ? [r?.push, r?.create].filter((c) => !!c) : [], true);
				set(refusal, null);
				await read.reload();
			} catch (e) {
				if (at !== get(key)) return;
				set(commands, [], true);
				const body = e instanceof Refused ? e.body : null;
				if (body?.refused === "weakened_unseen") {
					set(said, "");
					set(refusal, {
						says: body.says ?? "",
						rows: body.rows ?? [],
						seen_with: body.seen_with ?? ""
					}, true);
				} else {
					const f = failure(e, `devplane change show ${at}`);
					set(said, `That did not land: ${f.says}. \`${f.tell}\` tells more.`);
				}
			}
		});
	}
	async function openIn(place) {
		try {
			const r = await api(`/api/changes/${encodeURIComponent(get(key))}/open?in=${place}`, { method: "POST" });
			const where = r.path ?? get(d)?.worktree ?? "";
			set(said, r.opened ? `Opened ${where || "the worktree"} in the ${place}.` : r.says ?? (where ? `Open it yourself: ${where}` : `It was not opened in the ${place}.`), true);
		} catch (e) {
			set(said, `That did not land: ${failure(e).says}`);
		}
	}
	async function copy(text, what) {
		set(said, await copyText(text) ? `Copied ${what}.` : `Nothing was copied — this page has no clipboard. ${what}: ${text}`, true);
	}
	const tabs = /* @__PURE__ */ user_derived(() => [
		{
			id: "overview",
			label: "Overview",
			icon: "eye"
		},
		{
			id: "tasks",
			label: "Tasks",
			icon: "spec",
			count: get(d)?.counts?.tasks ?? null
		},
		{
			id: "review",
			label: "Review",
			icon: "split",
			count: get(d)?.review_files ?? null
		},
		{
			id: "gates",
			label: "Gates",
			icon: "gate",
			count: get(d)?.gates?.length ?? null,
			tone: get(d)?.gate ? get(verified) ? "done" : get(d).gate.passed ? void 0 : "fail" : void 0
		},
		{
			id: "ledger",
			label: "Ledger",
			icon: "ledger"
		},
		{
			id: "agent",
			label: "Agent",
			icon: "agent",
			count: get(d)?.runs?.length ?? null
		}
	]);
	user_effect(() => onAction("review", () => {
		set(view, "review");
		return true;
	}));
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		{
			let $0 = /* @__PURE__ */ user_derived(() => loaded() ? "Pick a change" : "Reading the changes…");
			Empty($$anchor, {
				icon: "change",
				get title() {
					return get($0);
				},
				body: "Choose one in the list, or start a new one — an isolated worktree, an agent in it, and the project's own gates."
			});
		}
	};
	var consequent_1 = ($$anchor) => {
		var div = root$22();
		Failed(child(div), {
			what: "this change",
			get failure() {
				return read.failure;
			}
		});
		reset(div);
		append($$anchor, div);
	};
	var consequent_2 = ($$anchor) => {
		append($$anchor, root_1$22());
	};
	var alternate_1 = ($$anchor) => {
		var article = root_16$2();
		var header = child(article);
		var div_2 = child(header);
		var h1 = child(div_2);
		var text_1 = only_child(h1, true);
		var node_2 = sibling(h1, 2);
		Pill(node_2, { get word() {
			return get(d).state;
		} });
		var node_3 = sibling(node_2);
		Qualifier(node_3, { get q() {
			return get(d).qualifier;
		} });
		var node_4 = sibling(node_3, 2);
		var consequent_3 = ($$anchor) => {
			Pill($$anchor, {
				get word() {
					return get(d).waiting_says;
				},
				as: "wait"
			});
		};
		if_block(node_4, ($$render) => {
			if (get(d).waiting_says) $$render(consequent_3);
		});
		reset(div_2);
		var div_3 = sibling(div_2, 2);
		var span = child(div_3);
		var node_5 = child(span);
		Icon(node_5, {
			name: "folder",
			size: 12
		});
		var text_2 = sibling(node_5);
		reset(span);
		var node_6 = sibling(span, 2);
		var consequent_4 = ($$anchor) => {
			var button = root_2$18();
			var node_7 = child(button);
			Icon(node_7, {
				name: "change",
				size: 12
			});
			var text_3 = sibling(node_7);
			reset(button);
			template_effect(() => set_text(text_3, ` ${get(d).branch ?? ""}`));
			delegated("click", button, () => copy(get(d).branch, "the branch name"));
			append($$anchor, button);
		};
		if_block(node_6, ($$render) => {
			if (get(d).branch) $$render(consequent_4);
		});
		var node_8 = sibling(node_6, 2);
		var consequent_5 = ($$anchor) => {
			var span_1 = root_3$16();
			var node_9 = child(span_1);
			Icon(node_9, {
				name: "spec",
				size: 12
			});
			var text_4 = sibling(node_9);
			reset(span_1);
			template_effect(() => set_text(text_4, ` ${get(d).spec ?? ""}`));
			append($$anchor, span_1);
		};
		if_block(node_8, ($$render) => {
			if (get(d).spec) $$render(consequent_5);
		});
		var node_10 = sibling(node_8, 2);
		var consequent_6 = ($$anchor) => {
			var span_2 = root_4$15();
			Icon(child(span_2), {
				name: "alert",
				size: 12
			});
			next();
			reset(span_2);
			append($$anchor, span_2);
		};
		if_block(node_10, ($$render) => {
			if (get(d).in_place) $$render(consequent_6);
		});
		var node_12 = sibling(node_10, 2);
		var consequent_7 = ($$anchor) => {
			var span_3 = root_5$13();
			var text_5 = only_child(span_3, true);
			template_effect(() => set_text(text_5, get(d).shape_says));
			append($$anchor, span_3);
		};
		if_block(node_12, ($$render) => {
			if (get(d).shape_says) $$render(consequent_7);
		});
		reset(div_3);
		var p = sibling(div_3, 2);
		Inline(child(p), { get text() {
			return get(d).standing_says;
		} });
		reset(p);
		var div_4 = sibling(p, 2);
		var button_1 = child(div_4);
		let classes;
		var node_14 = child(button_1);
		Icon(node_14, {
			name: "split",
			size: 14
		});
		var node_15 = sibling(node_14, 2);
		var consequent_8 = ($$anchor) => {
			var span_4 = root_6$12();
			var text_6 = only_child(span_4);
			template_effect(() => set_text(text_6, `${get(unseen) ?? ""} unseen`));
			append($$anchor, span_4);
		};
		if_block(node_15, ($$render) => {
			if (get(unseen) > 0) $$render(consequent_8);
		});
		reset(button_1);
		var node_16 = sibling(button_1, 2);
		var consequent_9 = ($$anchor) => {
			var button_2 = root_7$12();
			Icon(child(button_2), {
				name: "gate",
				size: 14
			});
			next();
			reset(button_2);
			template_effect(() => button_2.disabled = !!pending.busy);
			delegated("click", button_2, () => act("verify"));
			append($$anchor, button_2);
		};
		if_block(node_16, ($$render) => {
			if (get(d).worktree) $$render(consequent_9);
		});
		var node_18 = sibling(node_16, 2);
		var consequent_10 = ($$anchor) => {
			var button_3 = root_8$12();
			let classes_1;
			Icon(child(button_3), {
				name: "forge",
				size: 14
			});
			next();
			reset(button_3);
			template_effect(() => {
				button_3.disabled = !!pending.busy;
				classes_1 = set_class(button_3, 1, "svelte-178dv07", null, classes_1, { primary: get(unseen) === 0 });
			});
			delegated("click", button_3, () => act("offer"));
			append($$anchor, button_3);
		};
		if_block(node_18, ($$render) => {
			if (get(verified)) $$render(consequent_10);
		});
		var node_20 = sibling(node_18, 2);
		var consequent_12 = ($$anchor) => {
			var fragment_3 = root_10$10();
			var button_4 = first_child(fragment_3);
			Icon(child(button_4), {
				name: "play",
				size: 14
			});
			next();
			reset(button_4);
			var node_22 = sibling(button_4, 2);
			var consequent_11 = ($$anchor) => {
				var button_5 = root_9$10();
				Icon(child(button_5), {
					name: "refresh",
					size: 14
				});
				next();
				reset(button_5);
				template_effect(() => button_5.disabled = !!pending.busy);
				delegated("click", button_5, () => act("retry"));
				append($$anchor, button_5);
			};
			if_block(node_22, ($$render) => {
				if (get(d).can_retry) $$render(consequent_11);
			});
			template_effect(() => button_4.disabled = !!pending.busy);
			delegated("click", button_4, () => act("resume"));
			append($$anchor, fragment_3);
		};
		if_block(node_20, ($$render) => {
			if (get(d).stopped) $$render(consequent_12);
		});
		var node_24 = sibling(node_20, 2);
		var consequent_13 = ($$anchor) => {
			var button_6 = root_11$7();
			Icon(child(button_6), {
				name: "check",
				size: 14
			});
			next();
			reset(button_6);
			template_effect(() => button_6.disabled = !!pending.busy);
			delegated("click", button_6, () => act("finish"));
			append($$anchor, button_6);
		};
		if_block(node_24, ($$render) => {
			if (!get(d).completion && !get(d).archived_at && get(d).runs?.length) $$render(consequent_13);
		});
		var node_26 = sibling(node_24, 4);
		var consequent_14 = ($$anchor) => {
			var fragment_4 = root_12$5();
			var button_7 = first_child(fragment_4);
			Icon(child(button_7), {
				name: "external",
				size: 14
			});
			next();
			reset(button_7);
			var button_8 = sibling(button_7, 2);
			Icon(child(button_8), {
				name: "terminal",
				size: 14
			});
			next();
			reset(button_8);
			delegated("click", button_7, () => openIn("editor"));
			delegated("click", button_8, () => openIn("terminal"));
			append($$anchor, fragment_4);
		};
		if_block(node_26, ($$render) => {
			if (get(d).worktree) $$render(consequent_14);
		});
		var node_29 = sibling(node_26, 2);
		var consequent_15 = ($$anchor) => {
			var button_9 = root_13$5();
			Icon(child(button_9), {
				name: "folder",
				size: 14
			});
			next();
			reset(button_9);
			template_effect(() => button_9.disabled = !!pending.busy);
			delegated("click", button_9, () => act("archive"));
			append($$anchor, button_9);
		};
		if_block(node_29, ($$render) => {
			if (!get(d).archived_at && (get(d).completion || get(d).stopped)) $$render(consequent_15);
		});
		reset(div_4);
		var p_1 = sibling(div_4, 2);
		let classes_2;
		var node_31 = child(p_1);
		var consequent_16 = ($$anchor) => {
			var text_7 = text();
			template_effect(() => set_text(text_7, `${DOING[pending.busy] ?? ""} · ${pending.elapsed ?? ""} — nothing else can be started on this change until it is back.`));
			append($$anchor, text_7);
		};
		var alternate = ($$anchor) => {
			var text_8 = text();
			template_effect(() => set_text(text_8, get(said)));
			append($$anchor, text_8);
		};
		if_block(node_31, ($$render) => {
			if (pending.busy) $$render(consequent_16);
			else $$render(alternate, -1);
		});
		reset(p_1);
		var node_32 = sibling(p_1, 2);
		var consequent_17 = ($$anchor) => {
			Commands($$anchor, { get commands() {
				return get(commands);
			} });
		};
		if_block(node_32, ($$render) => {
			if (get(commands).length && !pending.busy) $$render(consequent_17);
		});
		var node_33 = sibling(node_32, 2);
		var consequent_18 = ($$anchor) => {
			Failed($$anchor, {
				what: "this change",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				stale: true
			});
		};
		if_block(node_33, ($$render) => {
			if (read.phase === "stale" && read.failure) $$render(consequent_18);
		});
		var node_34 = sibling(node_33, 2);
		var consequent_19 = ($$anchor) => {
			var div_5 = root_15$4();
			var p_2 = child(div_5);
			var text_9 = sibling(child(p_2));
			reset(p_2);
			var ul = sibling(p_2, 2);
			each(ul, 21, () => get(refusal).rows, (row) => row.path + row.matched, ($$anchor, row) => {
				var li = root_14$5();
				var code = child(li);
				var text_10 = only_child(code, true);
				var span_5 = sibling(code, 2);
				Inline(child(span_5), { get text() {
					return get(row).why;
				} });
				reset(span_5);
				var button_10 = sibling(span_5, 2);
				Icon(child(button_10), {
					name: "split",
					size: 13
				});
				next();
				reset(button_10);
				reset(li);
				template_effect(() => set_text(text_10, get(row).path));
				delegated("click", button_10, () => readRow(get(row).path));
				append($$anchor, li);
			});
			reset(ul);
			var p_3 = sibling(ul, 2);
			var text_11 = only_child(sibling(child(p_3)), true);
			reset(p_3);
			reset(div_5);
			template_effect(() => {
				set_text(text_9, ` — ${get(refusal).says ?? ""}. Nothing was pushed.`);
				set_text(text_11, get(refusal).seen_with);
			});
			append($$anchor, div_5);
		};
		if_block(node_34, ($$render) => {
			if (get(refusal)) $$render(consequent_19);
		});
		var node_37 = sibling(node_34, 2);
		{
			let $0 = /* @__PURE__ */ user_derived(() => !!get(d).archived_at);
			let $1 = /* @__PURE__ */ user_derived(() => !!get(d).pull_request);
			Stepper(node_37, {
				get state() {
					return get(d).state;
				},
				get archived() {
					return get($0);
				},
				get offered() {
					return get($1);
				}
			});
		}
		reset(header);
		var node_38 = sibling(header, 2);
		Tabs(node_38, {
			get tabs() {
				return get(tabs);
			},
			label: "views of this change",
			get active() {
				return get(view);
			},
			set active($$value) {
				set(view, $$value, true);
			}
		});
		var div_6 = sibling(node_38, 2);
		var node_39 = child(div_6);
		var consequent_20 = ($$anchor) => {
			Overview($$anchor, {
				get d() {
					return get(d);
				},
				go: (v) => set(view, v, true)
			});
		};
		var consequent_21 = ($$anchor) => {
			Tasks($$anchor, {
				get d() {
					return get(d);
				},
				get lastRun() {
					return get(lastRun);
				},
				get reload() {
					return read.reload;
				}
			});
		};
		var consequent_22 = ($$anchor) => {
			Pane($$anchor, {
				get id() {
					return get(key);
				},
				get focus() {
					return get(reviewAt);
				}
			});
		};
		var consequent_23 = ($$anchor) => {
			Gates($$anchor, {
				get d() {
					return get(d);
				},
				get id() {
					return get(key);
				}
			});
		};
		var consequent_24 = ($$anchor) => {
			Ledger($$anchor, { get id() {
				return get(key);
			} });
		};
		var consequent_25 = ($$anchor) => {
			Agent($$anchor, {
				get d() {
					return get(d);
				},
				get run() {
					return get(lastRun);
				}
			});
		};
		if_block(node_39, ($$render) => {
			if (get(view) === "overview") $$render(consequent_20);
			else if (get(view) === "tasks") $$render(consequent_21, 1);
			else if (get(view) === "review") $$render(consequent_22, 2);
			else if (get(view) === "gates") $$render(consequent_23, 3);
			else if (get(view) === "ledger") $$render(consequent_24, 4);
			else if (get(view) === "agent") $$render(consequent_25, 5);
		});
		reset(div_6);
		reset(article);
		template_effect(($0) => {
			set_text(text_1, get(d).title || get(d).id);
			set_text(text_2, ` ${get(project) ?? ""}`);
			set_class(p, 1, `standing ${get(verified) ? "done" : ""}`, "svelte-178dv07");
			classes = set_class(button_1, 1, "svelte-178dv07", null, classes, { primary: get(unseen) > 0 || !get(verified) });
			classes_2 = set_class(p_1, 1, "said svelte-178dv07", null, classes_2, { empty: !get(said) && !pending.busy });
			set_attribute(div_6, "aria-label", $0);
		}, [() => get(tabs).find((t) => t.id === get(view))?.label]);
		delegated("click", button_1, () => set(view, "review"));
		append($$anchor, article);
	};
	if_block(node, ($$render) => {
		if (!id()) $$render(consequent);
		else if (read.failure && !get(d)) $$render(consequent_1, 1);
		else if (!get(d)) $$render(consequent_2, 2);
		else $$render(alternate_1, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/index.ts
bind({
	surface: "global",
	combo: "g c",
	action: "go-changes",
	label: "go to the changes"
});
bind({
	surface: "change",
	combo: "r",
	action: "review",
	label: "review the change"
});
onAction("go-changes", () => {
	go("#change");
	return true;
});
var briefs = (feed) => feed.board?.changes ?? [];
register({
	id: "change",
	icon: "change",
	title: "Changes",
	heading: "Changes",
	band: "attention",
	order: 1,
	link: "change",
	count: (feed) => {
		const b = feed.board;
		return b?.changes ? b.changes.filter((c) => !isArchived(c.state)).length : null;
	},
	tab: (feed, focus) => briefs(feed).find((c) => c.id === focus)?.title ?? "Change",
	select: (feed, focus) => {
		const all = briefs(feed);
		return {
			id: focus,
			all,
			brief: all.find((c) => c.id === focus) ?? null,
			loaded: phase(feed).loaded
		};
	},
	side: List$2,
	component: Doc$1
});
//#endregion
//#region src/surfaces/github/GithubList.svelte
var act = ($$anchor, kind = noop) => {
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var a = root$21();
		Icon(child(a), {
			name: "person",
			size: 13
		});
		next();
		reset(a);
		append($$anchor, a);
	};
	var consequent_1 = ($$anchor) => {
		append($$anchor, root_1$21());
	};
	var consequent_2 = ($$anchor) => {
		append($$anchor, root_2$17());
	};
	var alternate = ($$anchor) => {
		append($$anchor, root_3$15());
	};
	if_block(node, ($$render) => {
		if (kind() === "sign_in") $$render(consequent);
		else if (kind() === "wait") $$render(consequent_1, 1);
		else if (kind() === "doctor") $$render(consequent_2, 2);
		else $$render(alternate, -1);
	});
	append($$anchor, fragment);
};
function stateSays(g, now = Date.now()) {
	if (!g) return null;
	switch (g.state) {
		case "signed_out": return {
			title: "Not signed in to GitHub",
			body: g.said ? g.said : "Nothing is read from GitHub until you sign in; the token is kept only in this machine's credential store.",
			action: "sign_in",
			blocks: true
		};
		case "pending": return {
			title: "Signing in to GitHub",
			body: `Enter the code ${g.user_code ?? ""} at ${g.verification_uri ?? "GitHub's device page"}.`,
			action: "sign_in",
			blocks: true
		};
		case "expired": return {
			title: "GitHub sign-in expired",
			body: "GitHub no longer accepts the token, so it was removed from this machine.",
			action: "sign_in",
			blocks: true
		};
		case "rate_limited": return {
			title: "GitHub's rate limit is spent",
			body: `It resets at ${clock(g.until)}; Devplane asks again then.`,
			action: "wait",
			blocks: false
		};
		case "unreachable": return {
			title: "GitHub unreachable",
			body: `${g.why ? `${g.why}. ` : ""}${g.since ? `Since ${clock(g.since)}.` : ""}`.trim(),
			action: "doctor",
			blocks: false
		};
		case "not_github": return {
			title: "Not a GitHub repository",
			body: g.why ?? "Its git remote is not on a GitHub host this machine signs in to.",
			action: "remote",
			blocks: true
		};
		default: return null;
	}
}
function clock(at) {
	if (!at) return "a time GitHub did not say";
	const d = new Date(at);
	if (Number.isNaN(d.getTime())) return at;
	return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}
var root$21 = /* @__PURE__ */ from_html(`<a class="act svelte-sff017" href="#setup"><!> Sign in</a>`);
var root_1$21 = /* @__PURE__ */ from_html(`<span class="quiet svelte-sff017">nothing to do — it resumes on its own</span>`);
var root_2$17 = /* @__PURE__ */ from_html(`<span class="quiet svelte-sff017">check the network, then <code class="svelte-sff017">devplane doctor</code></span>`);
var root_3$15 = /* @__PURE__ */ from_html(`<span class="quiet svelte-sff017">add a GitHub remote: <code class="svelte-sff017">git remote add origin git@github.com:owner/name.git</code></span>`);
var root_4$14 = /* @__PURE__ */ from_html(`<span class="cov svelte-sff017"> </span>`);
var root_5$12 = /* @__PURE__ */ from_html(`<p class="quiet svelte-sff017">Asking GitHub…</p>`);
var root_6$11 = /* @__PURE__ */ from_html(`<div class="state"><!></div>`);
var root_7$11 = /* @__PURE__ */ from_html(`<div class="state" data-state="not_github"><!></div>`);
var root_8$11 = /* @__PURE__ */ from_html(`<span class="stale svelte-sff017"> </span>`);
var root_9$9 = /* @__PURE__ */ from_html(`<p class="banner svelte-sff017"><!> <b> </b> <!> <!></p>`);
var root_10$9 = /* @__PURE__ */ from_html(`<p class="quiet svelte-sff017"> </p>`);
var root_11$6 = /* @__PURE__ */ from_html(`<p class="quiet svelte-sff017">Nothing was read before GitHub stopped answering, so there is no list to show.</p>`);
var root_12$4 = /* @__PURE__ */ from_html(`<span class="you svelte-sff017" title="needs you"><!></span>`);
var root_13$4 = /* @__PURE__ */ from_html(`<span class="t svelte-sff017"> </span>`);
var root_14$4 = /* @__PURE__ */ from_html(`<span class="dim svelte-sff017"> </span>`);
var root_15$3 = /* @__PURE__ */ from_html(`<a target="_blank" rel="noopener noreferrer" class="open svelte-sff017"><!> open</a>`);
var root_16$1 = /* @__PURE__ */ from_html(`<p class="quiet more svelte-sff017"> </p>`);
var root_17$1 = /* @__PURE__ */ from_html(`<div class="frame svelte-sff017"><!></div> <!>`, 1);
var root_18 = /* @__PURE__ */ from_html(`<!> <!> <!>`, 1);
var root_19 = /* @__PURE__ */ from_html(`<div class="page svelte-sff017"><header class="head svelte-sff017"><h1 class="svelte-sff017">Issues and pull requests</h1> <!></header> <!> <!></div>`);
function GithubList($$anchor, $$props) {
	push($$props, true);
	let issues = prop($$props, "issues", 19, () => []), pulls = prop($$props, "pulls", 19, () => []), loaded = prop($$props, "loaded", 3, false), failed = prop($$props, "failed", 3, ""), tab = prop($$props, "tab", 3, "issues"), coverage = prop($$props, "coverage", 3, null), github = prop($$props, "github", 3, null), projects = prop($$props, "projects", 19, () => []), readAt = prop($$props, "readAt", 3, null);
	let showing = /* @__PURE__ */ state("issues");
	user_effect(() => {
		set(showing, tab());
	});
	const shown = /* @__PURE__ */ user_derived(() => get(showing) === "issues" ? issues() : pulls());
	const absent = /* @__PURE__ */ user_derived(() => coverage() ? Math.max(0, coverage().projects - coverage().configured) : 0);
	const says = /* @__PURE__ */ user_derived(() => stateSays(github()));
	const elsewhere = /* @__PURE__ */ user_derived(() => projects().filter((p) => p.github?.state === "not_github"));
	const onGithub = /* @__PURE__ */ user_derived(() => projects().filter((p) => p.github?.state !== "not_github"));
	const allElsewhere = /* @__PURE__ */ user_derived(() => projects().length > 0 && get(onGithub).length === 0);
	const blocking = /* @__PURE__ */ user_derived(() => !!get(says)?.blocks);
	const stale = /* @__PURE__ */ user_derived(() => !!get(says) && !get(says).blocks);
	const more = /* @__PURE__ */ user_derived(() => projects().reduce((n, p) => n + ((get(showing) === "issues" ? p.issues_more : p.pull_requests_more) ?? 0), 0));
	const age = /* @__PURE__ */ user_derived(() => readAt() ? ago$1((Date.now() - new Date(readAt()).getTime()) / 1e3) : "");
	const counted = /* @__PURE__ */ user_derived(() => loaded() && !failed() && !get(blocking) && !get(allElsewhere) && !get(stale));
	let selected = /* @__PURE__ */ state(null);
	const columns = [
		{
			key: "you",
			label: "",
			width: 36
		},
		{
			key: "title",
			label: "Title",
			width: 520,
			sort: (r) => r.title
		},
		{
			key: "project",
			label: "Project",
			width: 160,
			sort: (r) => r.project_name
		},
		{
			key: "open",
			label: "",
			align: "end"
		}
	];
	var div = root_19();
	var header = child(div);
	var node_2 = sibling(child(header), 2);
	var consequent_3 = ($$anchor) => {
		var span_3 = root_4$14();
		var text = only_child(span_3);
		template_effect(() => set_text(text, `${coverage().configured ?? ""} of ${coverage().projects ?? ""} projects are on GitHub${get(absent) ? ` — ${get(absent)} not seen here` : ""}`));
		append($$anchor, span_3);
	};
	if_block(node_2, ($$render) => {
		if (coverage() && !get(blocking)) $$render(consequent_3);
	});
	reset(header);
	var node_3 = sibling(header, 2);
	{
		let $0 = /* @__PURE__ */ user_derived(() => [{
			id: "issues",
			label: "Issues",
			icon: "dot",
			count: get(counted) ? issues().length : null
		}, {
			id: "pulls",
			label: "Pull requests",
			icon: "forge",
			count: get(counted) ? pulls().length : null
		}]);
		Tabs(node_3, {
			get tabs() {
				return get($0);
			},
			label: "what to show",
			get active() {
				return get(showing);
			},
			set active($$value) {
				set(showing, $$value, true);
			}
		});
	}
	var node_4 = sibling(node_3, 2);
	var consequent_4 = ($$anchor) => {
		append($$anchor, root_5$12());
	};
	var consequent_5 = ($$anchor) => {
		Empty($$anchor, {
			icon: "alert",
			title: "The forge could not be read",
			get body() {
				return failed();
			}
		});
	};
	var consequent_6 = ($$anchor) => {
		var div_1 = root_6$11();
		var node_5 = child(div_1);
		{
			const action = ($$anchor) => {
				act($$anchor, () => get(says).action);
			};
			Empty(node_5, {
				icon: "alert",
				get title() {
					return get(says).title;
				},
				get body() {
					return get(says).body;
				},
				action,
				$$slots: { action: true }
			});
		}
		reset(div_1);
		template_effect(() => set_attribute(div_1, "data-state", github()?.state));
		append($$anchor, div_1);
	};
	var consequent_7 = ($$anchor) => {
		var div_2 = root_7$11();
		var node_6 = child(div_2);
		{
			const action = ($$anchor) => {
				act($$anchor, () => "remote");
			};
			let $0 = /* @__PURE__ */ user_derived(() => `${listed(get(elsewhere).map((p) => p.project_name))} ${plural(get(elsewhere).length, "has", "have")} no remote on a GitHub host this machine signs in to.`);
			Empty(node_6, {
				icon: "forge",
				title: "Not a GitHub repository",
				get body() {
					return get($0);
				},
				action,
				$$slots: { action: true }
			});
		}
		reset(div_2);
		append($$anchor, div_2);
	};
	var alternate_2 = ($$anchor) => {
		var fragment_4 = root_18();
		var node_7 = first_child(fragment_4);
		var consequent_9 = ($$anchor) => {
			var p_2 = root_9$9();
			var node_8 = child(p_2);
			Icon(node_8, {
				name: "alert",
				size: 13
			});
			var b = sibling(node_8, 2);
			var text_1 = only_child(b, true);
			var text_2 = sibling(b);
			var node_9 = sibling(text_2);
			var consequent_8 = ($$anchor) => {
				var span_4 = root_8$11();
				var text_3 = only_child(span_4);
				template_effect(() => set_text(text_3, `stale, read ${get(age) ?? ""} ago`));
				append($$anchor, span_4);
			};
			if_block(node_9, ($$render) => {
				if (get(age)) $$render(consequent_8);
			});
			act(sibling(node_9, 2), () => get(says).action);
			reset(p_2);
			template_effect(() => {
				set_attribute(p_2, "data-state", github()?.state);
				set_text(text_1, get(says).title);
				set_text(text_2, ` — ${get(says).body ?? ""} `);
			});
			append($$anchor, p_2);
		};
		if_block(node_7, ($$render) => {
			if (get(says) && get(stale)) $$render(consequent_9);
		});
		var node_11 = sibling(node_7, 2);
		var consequent_10 = ($$anchor) => {
			var p_3 = root_10$9();
			var text_4 = only_child(p_3);
			template_effect(($0) => set_text(text_4, `${$0 ?? ""}: not a GitHub repository.`), [() => listed(get(elsewhere).map((p) => p.project_name))]);
			append($$anchor, p_3);
		};
		if_block(node_11, ($$render) => {
			if (get(elsewhere).length) $$render(consequent_10);
		});
		var node_12 = sibling(node_11, 2);
		var consequent_11 = ($$anchor) => {
			append($$anchor, root_11$6());
		};
		var consequent_12 = ($$anchor) => {
			{
				let $0 = /* @__PURE__ */ user_derived(() => get(showing) === "issues" ? "No open issue" : "No open pull request");
				let $1 = /* @__PURE__ */ user_derived(() => get(absent) ? `${get(absent)} projects have no GitHub remote and are not counted.` : "");
				Empty($$anchor, {
					icon: "forge",
					get title() {
						return get($0);
					},
					body: "Across every registered project that has a GitHub remote.",
					get limit() {
						return get($1);
					}
				});
			}
		};
		var alternate_1 = ($$anchor) => {
			var fragment_6 = root_17$1();
			var div_3 = first_child(fragment_6);
			var node_13 = child(div_3);
			{
				const cell = ($$anchor, r = noop, c = noop) => {
					var fragment_7 = comment();
					var node_14 = first_child(fragment_7);
					var consequent_14 = ($$anchor) => {
						var fragment_8 = comment();
						var node_15 = first_child(fragment_8);
						var consequent_13 = ($$anchor) => {
							var span_5 = root_12$4();
							Icon(child(span_5), {
								name: "person",
								size: 13
							});
							reset(span_5);
							append($$anchor, span_5);
						};
						if_block(node_15, ($$render) => {
							if (r().needs_you) $$render(consequent_13);
						});
						append($$anchor, fragment_8);
					};
					var consequent_15 = ($$anchor) => {
						var span_6 = root_13$4();
						var text_5 = only_child(span_6, true);
						template_effect(() => set_text(text_5, r().title));
						append($$anchor, span_6);
					};
					var consequent_16 = ($$anchor) => {
						var span_7 = root_14$4();
						var text_6 = only_child(span_7, true);
						template_effect(() => set_text(text_6, r().project_name));
						append($$anchor, span_7);
					};
					var consequent_17 = ($$anchor) => {
						var a_1 = root_15$3();
						Icon(child(a_1), {
							name: "external",
							size: 13
						});
						next();
						reset(a_1);
						template_effect(($0) => set_attribute(a_1, "href", $0), [() => safeHref(r().url)]);
						append($$anchor, a_1);
					};
					var d_1 = /* @__PURE__ */ user_derived(() => safeHref(r().url));
					if_block(node_14, ($$render) => {
						if (c().key === "you") $$render(consequent_14);
						else if (c().key === "title") $$render(consequent_15, 1);
						else if (c().key === "project") $$render(consequent_16, 2);
						else if (get(d_1)) $$render(consequent_17, 3);
					});
					append($$anchor, fragment_7);
				};
				Grid(node_13, {
					id: "forge",
					get columns() {
						return columns;
					},
					get rows() {
						return get(shown);
					},
					key: (r) => r.url,
					group: (r) => r.needs_you ? "Needs you" : "Everything else",
					open: (r) => {
						const to = safeHref(r.url);
						if (to) window.open(to, "_blank", "noopener");
					},
					get label() {
						return get(showing);
					},
					get selected() {
						return get(selected);
					},
					set selected($$value) {
						set(selected, $$value, true);
					},
					cell,
					$$slots: { cell: true }
				});
			}
			reset(div_3);
			var node_18 = sibling(div_3, 2);
			var consequent_18 = ($$anchor) => {
				var p_5 = root_16$1();
				var text_7 = only_child(p_5);
				template_effect(($0) => set_text(text_7, `${get(more) ?? ""} more open ${$0 ?? ""} on GitHub — only the newest are read each time.`), [() => get(showing) === "issues" ? plural(get(more), "issue", "issues") : plural(get(more), "pull request", "pull requests")]);
				append($$anchor, p_5);
			};
			if_block(node_18, ($$render) => {
				if (get(more) > 0) $$render(consequent_18);
			});
			append($$anchor, fragment_6);
		};
		if_block(node_12, ($$render) => {
			if (get(shown).length === 0 && get(stale)) $$render(consequent_11);
			else if (get(shown).length === 0) $$render(consequent_12, 1);
			else $$render(alternate_1, -1);
		});
		append($$anchor, fragment_4);
	};
	if_block(node_4, ($$render) => {
		if (!loaded()) $$render(consequent_4);
		else if (failed()) $$render(consequent_5, 1);
		else if (get(says) && get(blocking)) $$render(consequent_6, 2);
		else if (get(allElsewhere)) $$render(consequent_7, 3);
		else $$render(alternate_2, -1);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
//#endregion
//#region src/surfaces/github/Github.svelte
var root$20 = /* @__PURE__ */ from_html(`<div class="stale svelte-1n13jp3"><!></div>`);
var root_1$20 = /* @__PURE__ */ from_html(`<!> <!>`, 1);
function Github($$anchor, $$props) {
	push($$props, true);
	let coverage = prop($$props, "coverage", 3, null);
	const read = resource(() => "/api/forge", {
		every: 3e4,
		tell: () => "devplane forge issues"
	});
	const issues = /* @__PURE__ */ user_derived(() => read.data?.issues ?? []);
	const pulls = /* @__PURE__ */ user_derived(() => read.data?.pull_requests ?? []);
	const loaded = /* @__PURE__ */ user_derived(() => read.phase !== "loading");
	const failed = /* @__PURE__ */ user_derived(() => read.data?.error ?? (read.phase === "failed" ? read.failure?.says ?? "" : ""));
	var fragment = root_1$20();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var div = root$20();
		Failed(child(div), {
			what: "the forge",
			get failure() {
				return read.failure;
			},
			get at() {
				return read.at;
			},
			stale: true
		});
		reset(div);
		append($$anchor, div);
	};
	if_block(node, ($$render) => {
		if (read.phase === "stale" && read.failure) $$render(consequent);
	});
	var node_2 = sibling(node, 2);
	{
		let $0 = /* @__PURE__ */ user_derived(() => read.data?.github ?? null);
		let $1 = /* @__PURE__ */ user_derived(() => read.data?.projects ?? []);
		let $2 = /* @__PURE__ */ user_derived(() => read.data?.fetched_at ?? null);
		GithubList(node_2, {
			get issues() {
				return get(issues);
			},
			get pulls() {
				return get(pulls);
			},
			get loaded() {
				return get(loaded);
			},
			get failed() {
				return get(failed);
			},
			get coverage() {
				return coverage();
			},
			get github() {
				return get($0);
			},
			get projects() {
				return get($1);
			},
			get readAt() {
				return get($2);
			}
		});
	}
	append($$anchor, fragment);
	pop();
}
//#endregion
//#region src/surfaces/github/index.ts
register({
	id: "github",
	icon: "forge",
	title: "Forge",
	heading: "Issues and pull requests",
	band: "steering",
	order: 1,
	count: (feed) => {
		const per = feed.board?.forge;
		if (!per) return null;
		return Object.values(per).reduce((n, f) => n + (f.needs_you ?? 0), 0);
	},
	reads: ["/api/forge"],
	select: (feed) => {
		const b = feed.board;
		if (!b?.projects) return {};
		return { coverage: {
			projects: b.projects.length,
			configured: Object.keys(b.forge ?? {}).length
		} };
	},
	component: Github
});
//#endregion
//#region src/lib/cursor.ts
function cursor(c) {
	const clamp = (i) => Math.min(Math.max(i, 0), c.size() - 1);
	const on = (action, move) => onAction(action, (surface) => {
		if (surface !== c.scope || c.size() === 0) return false;
		const to = move(c.at());
		if (to !== null) c.go(clamp(to));
		return true;
	});
	const offs = [
		on("next", (at) => at + 1),
		on("prev", (at) => at < 0 ? 0 : at - 1),
		on("first", () => 0),
		on("last", () => c.size() - 1),
		on("open", (at) => {
			c.open(clamp(at < 0 ? 0 : at));
			return null;
		})
	];
	return () => offs.forEach((off) => off());
}
//#endregion
//#region src/surfaces/inbox/place.svelte.ts
function address(focus, items) {
	if (focus.startsWith("item=")) return {
		item: focus.slice(5),
		wanted: "",
		project: ""
	};
	if (focus.startsWith("ask=")) return {
		item: "",
		wanted: focus.slice(4),
		project: ""
	};
	if (focus && items.some((i) => i.id === focus)) return {
		item: focus,
		wanted: "",
		project: ""
	};
	return {
		item: "",
		wanted: "",
		project: focus
	};
}
var itemFocus = (id) => `item=${id}`;
var last = proxy({
	id: "",
	index: 0
});
function remember(id, index) {
	if (last.id !== id || last.index !== index) {
		last.id = id;
		last.index = index;
	}
}
function place(shown, a) {
	if (a.item) {
		const i = shown.findIndex((x) => x.id === a.item);
		if (i !== -1) return {
			current: shown[i],
			gone: ""
		};
		const at = last.id === a.item ? Math.min(last.index, shown.length - 1) : 0;
		return {
			current: shown[Math.max(0, at)] ?? null,
			gone: "item"
		};
	}
	if (a.wanted) {
		const x = shown.find((y) => y.ask === a.wanted || y.request_id === a.wanted);
		return x ? {
			current: x,
			gone: ""
		} : {
			current: shown[0] ?? null,
			gone: "ask"
		};
	}
	return {
		current: shown[0] ?? null,
		gone: ""
	};
}
//#endregion
//#region src/lib/live.svelte.ts
var EVERY_MS$1 = 2e3;
var poke = null;
function reread() {
	poke?.();
}
function live(looking) {
	const state = proxy({
		board: null,
		inbox: null,
		loaded: false,
		stale_since: null,
		error: null,
		unauthorised: false
	});
	let inflight = false;
	let queued = null;
	let lastGood = null;
	let timer = null;
	async function tick(read) {
		if (state.unauthorised) return;
		if (inflight) {
			queued = (queued ?? false) || read;
			return;
		}
		inflight = true;
		try {
			const [board, inbox] = await Promise.all([api("/api/board"), api(read ? "/api/inbox?read=true" : "/api/inbox")]);
			if (!same$1(snapshot(state.board), board)) state.board = board;
			if (!same$1(snapshot(state.inbox), inbox)) state.inbox = inbox;
			state.loaded = true;
			state.error = null;
			state.stale_since = null;
			lastGood = (/* @__PURE__ */ new Date()).toISOString();
		} catch (e) {
			if (e instanceof Unauthorised) {
				state.unauthorised = true;
				state.error = "This tab has no token. Run `devplane open` again for a fresh link.";
				if (timer !== null) clearInterval(timer);
				timer = null;
				return;
			}
			state.error = e instanceof Error ? e.message : String(e);
			state.stale_since = lastGood;
		} finally {
			inflight = false;
			if (queued !== null && !state.unauthorised) {
				const again = queued;
				queued = null;
				tick(again);
			}
		}
	}
	function look() {
		tick(looking());
	}
	poke = () => void tick(false);
	function start() {
		look();
		timer = setInterval(() => {
			if (!document.hidden) tick(false);
		}, EVERY_MS$1);
		const onVisible = () => {
			if (document.visibilityState === "visible") look();
		};
		document.addEventListener("visibilitychange", onVisible);
		return () => {
			if (timer !== null) clearInterval(timer);
			timer = null;
			document.removeEventListener("visibilitychange", onVisible);
		};
	}
	return {
		state,
		start,
		look
	};
}
//#endregion
//#region src/surfaces/inbox/narrow.svelte.ts
var EVERY_MS = 2e3;
var shared = proxy({
	project: "",
	data: null,
	failure: null,
	at: null
});
var inflight = 0;
var seq = 0;
var landed = 0;
var users$1 = 0;
var timer$1 = null;
async function read$2(project, force = false) {
	if (inflight > 0 && !force) return;
	const n = ++seq;
	inflight += 1;
	try {
		const r = await api(`/api/inbox?project=${encodeURIComponent(project)}`);
		if (shared.project !== project || n < landed) return;
		landed = n;
		if (!same$1(snapshot(shared.data), r)) shared.data = r;
		shared.failure = null;
		shared.at = Date.now();
	} catch (e) {
		if (shared.project !== project || n < landed) return;
		landed = n;
		shared.failure = failure(e, `devplane inbox --project ${project}`);
	} finally {
		inflight = Math.max(0, inflight - 1);
	}
}
function narrowed(project) {
	const key = /* @__PURE__ */ user_derived(() => project().trim());
	user_effect(() => {
		const want = get(key);
		if (!want) return;
		if (shared.project !== want) {
			shared.project = want;
			shared.data = null;
			shared.failure = null;
			shared.at = null;
			landed = seq;
		}
		users$1 += 1;
		if (users$1 === 1 || shared.data === null) read$2(want);
		if (timer$1 === null) timer$1 = setInterval(() => {
			if (!document.hidden && shared.project) read$2(shared.project);
		}, EVERY_MS);
		return () => {
			users$1 -= 1;
			if (users$1 === 0 && timer$1 !== null) {
				clearInterval(timer$1);
				timer$1 = null;
			}
		};
	});
	const mine = () => !!get(key) && shared.project === get(key);
	return {
		get on() {
			return !!get(key);
		},
		get data() {
			return mine() ? shared.data : null;
		},
		get failure() {
			return mine() ? shared.failure : null;
		},
		get at() {
			return mine() ? shared.at : null;
		},
		get phase() {
			const d = mine() ? shared.data : null;
			const f = mine() ? shared.failure : null;
			return d ? f ? "stale" : "ok" : f ? "failed" : "loading";
		},
		get missing() {
			return mine() && shared.data?.narrowed?.no_such_project === true;
		},
		reload: () => get(key) ? read$2(get(key), true) : Promise.resolve()
	};
}
//#endregion
//#region src/surfaces/inbox/Inbox.svelte
var root$19 = /* @__PURE__ */ from_html(`<button class="undo svelte-ytvk4v"> </button>`);
var root_1$19 = /* @__PURE__ */ from_html(`<div class="missing svelte-ytvk4v" role="alert"><p class="svelte-ytvk4v"><b> </b> Nothing here is narrowed, and nothing here says the inbox is clear.</p> <button>Show the whole inbox</button></div>`);
var root_2$16 = /* @__PURE__ */ from_html(`<p class="dim stale svelte-ytvk4v">The last read had nothing for you, and Devplane has not answered since.</p>`);
var root_3$14 = /* @__PURE__ */ from_html(`<p class="svelte-ytvk4v">Nothing needed you, and nothing was decided for you.</p>`);
var root_4$13 = /* @__PURE__ */ from_html(`<p class="svelte-ytvk4v"> </p>`);
var root_5$11 = /* @__PURE__ */ from_html(`<p class="dim svelte-ytvk4v"> </p>`);
var root_6$10 = /* @__PURE__ */ from_html(`<div class="close svelte-ytvk4v"><span class="ok svelte-ytvk4v"><!></span> <h1 class="svelte-ytvk4v">Clear.</h1> <!> <!> <!> <!></div>`);
var root_7$10 = /* @__PURE__ */ from_html(`<p class="hairline stale svelte-ytvk4v"> </p>`);
var root_8$10 = /* @__PURE__ */ from_html(`<p class="gone svelte-ytvk4v" role="status"> </p>`);
var root_9$8 = /* @__PURE__ */ from_html(`<span class="proj svelte-ytvk4v"><!> </span>`);
var root_10$8 = /* @__PURE__ */ from_html(`<span class="age"> </span>`);
var root_11$5 = /* @__PURE__ */ from_html(`<a class="svelte-ytvk4v"><!> Open the change</a>`);
var root_12$3 = /* @__PURE__ */ from_html(`· <kbd class="svelte-ytvk4v">a</kbd> allow`, 1);
var root_13$3 = /* @__PURE__ */ from_html(`· <kbd class="svelte-ytvk4v">d</kbd> deny`, 1);
var root_14$3 = /* @__PURE__ */ from_html(`<!> <!> <header class="top svelte-ytvk4v"><span> </span> <span class="kind svelte-ytvk4v"> </span> <!> <!> <span class="pos svelte-ytvk4v"> </span></header> <fieldset class="bare svelte-ytvk4v"><ul role="list" tabindex="-1"><!></ul></fieldset> <footer class="nav svelte-ytvk4v"><!> <span class="keys svelte-ytvk4v"><kbd class="svelte-ytvk4v">j</kbd> <kbd class="svelte-ytvk4v">k</kbd> move between items · <kbd class="svelte-ytvk4v">Enter</kbd> moves into this one, <kbd class="svelte-ytvk4v">Tab</kbd> to its controls<!><!></span></footer>`, 1);
var root_15$2 = /* @__PURE__ */ from_html(`<section class="detail svelte-ytvk4v" aria-label="the item"><p role="status" aria-live="polite"> <!></p> <!> <!></section>`);
function Inbox($$anchor, $$props) {
	push($$props, true);
	let items = prop($$props, "items", 19, () => []), folded = prop($$props, "folded", 19, () => []), inhibited = prop($$props, "inhibited", 19, () => []), close = prop($$props, "close", 3, null), project = prop($$props, "project", 3, ""), item = prop($$props, "item", 3, ""), wanted = prop($$props, "wanted", 3, ""), loaded = prop($$props, "loaded", 3, false), stale_since = prop($$props, "stale_since", 3, null), error = prop($$props, "error", 3, null), watching = prop($$props, "watching", 3, null), open = prop($$props, "open", 3, () => {});
	const unseen = /* @__PURE__ */ user_derived(() => {
		const v = [...watching()?.driven_only ?? [], ...watching()?.unproved ?? []];
		return v.length <= 1 ? v.join("") : `${v.slice(0, -1).join(", ")} and ${v[v.length - 1]}`;
	});
	const narrow = narrowed(() => project());
	const shown = /* @__PURE__ */ user_derived(() => narrow.on ? narrow.data?.items ?? [] : items());
	const shownFolded = /* @__PURE__ */ user_derived(() => narrow.on ? narrow.data?.folded ?? [] : folded());
	const shownInhibited = /* @__PURE__ */ user_derived(() => narrow.on ? narrow.data?.inhibited ?? [] : inhibited());
	const shownClose = /* @__PURE__ */ user_derived(() => narrow.on ? null : close());
	const ready = /* @__PURE__ */ user_derived(() => narrow.on ? narrow.phase !== "loading" : loaded());
	const missing = /* @__PURE__ */ user_derived(() => narrow.missing);
	let said = /* @__PURE__ */ state(null);
	let undo = /* @__PURE__ */ state(null);
	const pending = writer();
	async function report(item, work) {
		const r = await pending.run("action", work);
		if (!r) return;
		set(said, {
			text: r.said,
			id: item.id,
			title: item.title,
			commands: r.commands
		}, true);
		set(undo, r.undo ? {
			...r.undo,
			id: item.id
		} : null, true);
		reread();
		narrow.reload();
	}
	const act = (item, action, reason) => report(item, () => act$1(item, action, reason));
	const answer$1 = (item, what) => report(item, () => answer(item, what));
	const snooze$1 = (item) => report(item, () => snooze(item));
	async function takeBack$1() {
		if (!get(undo)) return;
		const u = get(undo);
		set(undo, null);
		const r = await pending.run("undo", () => takeBack(u));
		if (r) set(said, {
			text: r.said,
			id: u.id,
			title: get(said)?.title ?? ""
		}, true);
		reread();
		narrow.reload();
	}
	async function copyRule$1(rule) {
		const item = get(current);
		const r = await copyRule(rule);
		if (item) set(said, {
			text: r.said,
			id: item.id,
			title: item.title
		}, true);
	}
	function say(text) {
		if (get(current)) set(said, {
			text,
			id: get(current).id,
			title: get(current).title
		}, true);
	}
	const nothingRaised = /* @__PURE__ */ user_derived(() => get(shown).length === 0 && get(shownFolded).length === 0 && get(shownInhibited).length === 0);
	const placed = /* @__PURE__ */ user_derived(() => place(get(shown), {
		item: item(),
		wanted: wanted()
	}));
	const current = /* @__PURE__ */ user_derived(() => get(placed).current);
	const position = /* @__PURE__ */ user_derived(() => Math.max(0, get(shown).findIndex((x) => x.id === get(current)?.id)));
	user_effect(() => {
		if (get(current) && get(current).id === item()) remember(item(), get(position));
	});
	const goneSays = /* @__PURE__ */ user_derived(() => !get(ready) || !get(placed).gone ? "" : get(placed).gone === "ask" ? "The ask you followed was already answered." : get(said)?.id === item() ? "" : "That item was resolved — this is the next one.");
	const sentence = /* @__PURE__ */ user_derived(() => {
		if (!get(said)) return "";
		if (get(said).id === get(current)?.id) return get(said).text;
		return get(shown).some((x) => x.id === get(said).id) ? "" : `${get(said).title}: ${get(said).text}`;
	});
	const undoHere = /* @__PURE__ */ user_derived(() => get(undo) && get(undo).id === get(current)?.id ? get(undo) : null);
	user_effect(() => cursor({
		scope: "inbox",
		size: () => get(shown).length,
		at: () => get(shown).findIndex((x) => x.id === get(current)?.id),
		go: (i) => open()(itemFocus(get(shown)[i].id)),
		open: () => document.querySelector(".detail .one")?.focus()
	}));
	function keyed(action) {
		return (surface) => {
			if (surface !== "inbox" || !get(current) || pending.busy || !(get(current).actions ?? []).includes(action)) return false;
			answer$1(get(current), { decision: action });
			return true;
		};
	}
	user_effect(() => {
		const offs = [onAction("inbox-allow", keyed("allow")), onAction("inbox-deny", keyed("deny"))];
		return () => offs.forEach((off) => off());
	});
	const keysHere = /* @__PURE__ */ user_derived(() => (get(current)?.actions ?? []).filter((a) => a === "allow" || a === "deny"));
	let now = /* @__PURE__ */ state(proxy(Date.now()));
	user_effect(() => {
		const id = setInterval(() => set(now, Date.now(), true), 3e4);
		return () => clearInterval(id);
	});
	var section = root_15$2();
	var p = child(section);
	let classes;
	var text_1 = child(p);
	var node = sibling(text_1);
	var consequent = ($$anchor) => {
		var button = root$19();
		var text_2 = only_child(button, true);
		template_effect(() => {
			button.disabled = !!pending.busy;
			set_text(text_2, get(undoHere).says);
		});
		delegated("click", button, takeBack$1);
		append($$anchor, button);
	};
	if_block(node, ($$render) => {
		if (get(undoHere)) $$render(consequent);
	});
	reset(p);
	var node_1 = sibling(p, 2);
	var consequent_1 = ($$anchor) => {
		Commands($$anchor, { get commands() {
			return get(said).commands;
		} });
	};
	if_block(node_1, ($$render) => {
		if (get(sentence) && get(said)?.commands?.length) $$render(consequent_1);
	});
	var node_2 = sibling(node_1, 2);
	var consequent_2 = ($$anchor) => {
		{
			let $0 = /* @__PURE__ */ user_derived(() => `the inbox narrowed to ${project()}`);
			Failed($$anchor, {
				get what() {
					return get($0);
				},
				get failure() {
					return narrow.failure;
				}
			});
		}
	};
	var consequent_3 = ($$anchor) => {
		var div = root_1$19();
		var p_1 = child(div);
		var text_3 = only_child(child(p_1));
		next();
		reset(p_1);
		var button_1 = sibling(p_1, 2);
		reset(div);
		template_effect(() => set_text(text_3, `No project is named “${project() ?? ""}”.`));
		delegated("click", button_1, () => open()(""));
		append($$anchor, div);
	};
	var consequent_4 = ($$anchor) => {
		Skeleton($$anchor, {});
	};
	var consequent_5 = ($$anchor) => {
		append($$anchor, root_2$16());
	};
	var consequent_10 = ($$anchor) => {
		var div_1 = root_6$10();
		var span = child(div_1);
		Icon(child(span), {
			name: "check",
			size: 22
		});
		reset(span);
		var node_4 = sibling(span, 4);
		var consequent_6 = ($$anchor) => {
			append($$anchor, root_3$14());
		};
		var alternate = ($$anchor) => {
			var fragment_3 = comment();
			each(first_child(fragment_3), 17, () => get(shownClose)?.sentences ?? [], index, ($$anchor, s) => {
				var p_4 = root_4$13();
				var text_4 = only_child(p_4, true);
				template_effect(() => set_text(text_4, get(s)));
				append($$anchor, p_4);
			});
			append($$anchor, fragment_3);
		};
		if_block(node_4, ($$render) => {
			if (get(shownClose)?.quiet) $$render(consequent_6);
			else $$render(alternate, -1);
		});
		var node_6 = sibling(node_4, 2);
		var consequent_7 = ($$anchor) => {
			var p_5 = root_5$11();
			var text_5 = only_child(p_5);
			template_effect(() => set_text(text_5, `Next · ${get(shownClose).next ?? ""}`));
			append($$anchor, p_5);
		};
		if_block(node_6, ($$render) => {
			if (get(shownClose)?.next) $$render(consequent_7);
		});
		var node_7 = sibling(node_6, 2);
		var consequent_8 = ($$anchor) => {
			var p_6 = root_5$11();
			var text_6 = only_child(p_6, true);
			template_effect(() => set_text(text_6, get(shownClose).keeps_running));
			append($$anchor, p_6);
		};
		if_block(node_7, ($$render) => {
			if (get(shownClose)?.keeps_running) $$render(consequent_8);
		});
		var node_8 = sibling(node_7, 2);
		var consequent_9 = ($$anchor) => {
			var p_7 = root_5$11();
			var text_7 = only_child(p_7);
			template_effect(() => set_text(text_7, `Sessions in ${get(unseen) ?? ""} are not on this board unless Devplane started them.`));
			append($$anchor, p_7);
		};
		if_block(node_8, ($$render) => {
			if (get(unseen)) $$render(consequent_9);
		});
		reset(div_1);
		append($$anchor, div_1);
	};
	var consequent_19 = ($$anchor) => {
		var fragment_4 = root_14$3();
		var node_9 = first_child(fragment_4);
		var consequent_11 = ($$anchor) => {
			{
				let $0 = /* @__PURE__ */ user_derived(() => `the inbox narrowed to ${project()}`);
				Failed($$anchor, {
					get what() {
						return get($0);
					},
					get failure() {
						return narrow.failure;
					},
					get at() {
						return narrow.at;
					},
					stale: true
				});
			}
		};
		var consequent_12 = ($$anchor) => {
			var p_8 = root_7$10();
			var text_8 = only_child(p_8);
			template_effect(($0) => set_text(text_8, `stale · as read ${$0 ?? ""}`), [() => stale_since() ? ago$1(Math.max(0, (Date.now() - Date.parse(stale_since())) / 1e3)) + " ago" : "before Devplane stopped answering"]);
			append($$anchor, p_8);
		};
		if_block(node_9, ($$render) => {
			if (narrow.on && narrow.failure) $$render(consequent_11);
			else if (error()) $$render(consequent_12, 1);
		});
		var node_10 = sibling(node_9, 2);
		var consequent_13 = ($$anchor) => {
			var p_9 = root_8$10();
			var text_9 = only_child(p_9, true);
			template_effect(() => set_text(text_9, get(goneSays)));
			append($$anchor, p_9);
		};
		if_block(node_10, ($$render) => {
			if (get(goneSays)) $$render(consequent_13);
		});
		var header = sibling(node_10, 2);
		var span_1 = child(header);
		var text_10 = only_child(span_1, true);
		var span_2 = sibling(span_1, 2);
		var text_11 = only_child(span_2, true);
		var node_11 = sibling(span_2, 2);
		var consequent_14 = ($$anchor) => {
			var span_3 = root_9$8();
			var node_12 = child(span_3);
			Icon(node_12, {
				name: "folder",
				size: 12
			});
			var text_12 = sibling(node_12);
			reset(span_3);
			template_effect(() => set_text(text_12, ` ${get(current).project_name ?? ""}`));
			append($$anchor, span_3);
		};
		if_block(node_11, ($$render) => {
			if (get(current).project_name) $$render(consequent_14);
		});
		var node_13 = sibling(node_11, 2);
		var consequent_15 = ($$anchor) => {
			var span_4 = root_10$8();
			var text_13 = only_child(span_4);
			template_effect(($0) => {
				set_attribute(span_4, "title", get(current).since);
				set_text(text_13, `${$0 ?? ""} ago`);
			}, [() => ago$1(Math.max(0, (get(now) - Date.parse(get(current).since)) / 1e3))]);
			append($$anchor, span_4);
		};
		if_block(node_13, ($$render) => {
			if (get(current).since) $$render(consequent_15);
		});
		var text_14 = only_child(sibling(node_13, 2));
		reset(header);
		var fieldset = sibling(header, 2);
		var ul = child(fieldset);
		let classes_1;
		key$1(child(ul), () => get(current).id, ($$anchor) => {
			Item($$anchor, {
				get item() {
					return get(current);
				},
				current: true,
				age: "",
				answer: answer$1,
				act,
				snooze: snooze$1,
				copyRule: copyRule$1,
				say
			});
		});
		reset(ul);
		reset(fieldset);
		var footer = sibling(fieldset, 2);
		var node_15 = child(footer);
		var consequent_16 = ($$anchor) => {
			var a_1 = root_11$5();
			Icon(child(a_1), {
				name: "change",
				size: 13
			});
			next();
			reset(a_1);
			template_effect(($0) => set_attribute(a_1, "href", $0), [() => `#change/${encodeURIComponent(get(current).change_id)}`]);
			append($$anchor, a_1);
		};
		if_block(node_15, ($$render) => {
			if (get(current).change_id) $$render(consequent_16);
		});
		var span_6 = sibling(node_15, 2);
		var node_17 = sibling(child(span_6), 8);
		var consequent_17 = ($$anchor) => {
			var fragment_7 = root_12$3();
			next(2);
			append($$anchor, fragment_7);
		};
		var d = /* @__PURE__ */ user_derived(() => get(keysHere).includes("allow"));
		if_block(node_17, ($$render) => {
			if (get(d)) $$render(consequent_17);
		});
		var node_18 = sibling(node_17);
		var consequent_18 = ($$anchor) => {
			var fragment_8 = root_13$3();
			next(2);
			append($$anchor, fragment_8);
		};
		var d_1 = /* @__PURE__ */ user_derived(() => get(keysHere).includes("deny"));
		if_block(node_18, ($$render) => {
			if (get(d_1)) $$render(consequent_18);
		});
		reset(span_6);
		reset(footer);
		template_effect(($0) => {
			set_class(span_1, 1, `lvl ${get(current).level ?? ""}`, "svelte-ytvk4v");
			set_text(text_10, get(current).level);
			set_text(text_11, $0);
			set_text(text_14, `${get(position) + 1} of ${get(shown).length ?? ""}`);
			fieldset.disabled = !!pending.busy;
			classes_1 = set_class(ul, 1, "one svelte-ytvk4v", null, classes_1, { stale: !!error() });
			set_attribute(ul, "aria-label", `the item: ${get(current).title ?? ""}`);
		}, [() => get(current).kind.replace(/_/g, " ")]);
		append($$anchor, fragment_4);
	};
	if_block(node_2, ($$render) => {
		if (narrow.on && narrow.failure && !narrow.data) $$render(consequent_2);
		else if (get(missing)) $$render(consequent_3, 1);
		else if (!get(ready) && get(nothingRaised)) $$render(consequent_4, 2);
		else if (get(nothingRaised) && error()) $$render(consequent_5, 3);
		else if (get(nothingRaised)) $$render(consequent_10, 4);
		else if (get(current)) $$render(consequent_19, 5);
	});
	reset(section);
	template_effect(() => {
		classes = set_class(p, 1, "said svelte-ytvk4v", null, classes, { empty: !get(sentence) && !get(undoHere) });
		set_text(text_1, `${get(sentence) ?? ""} `);
	});
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/inbox/List.svelte
var root$18 = /* @__PURE__ */ from_html(`<p class="since svelte-i436af"><!> </p>`);
var root_1$18 = /* @__PURE__ */ from_html(`<button> </button>`);
var root_2$15 = /* @__PURE__ */ from_html(`<div class="chips svelte-i436af" role="group" aria-label="narrow to one project"><button>all</button> <!></div>`);
var root_3$13 = /* @__PURE__ */ from_html(`<p class="quiet failed svelte-i436af"> </p>`);
var root_4$12 = /* @__PURE__ */ from_html(`<div class="skel svelte-i436af" aria-busy="true"></div>`);
var root_5$10 = /* @__PURE__ */ from_html(`<p class="quiet svelte-i436af">The last read had nothing for you, and Devplane has not answered since.</p>`);
var root_6$9 = /* @__PURE__ */ from_html(`<p class="quiet svelte-i436af"> </p>`);
var root_7$9 = /* @__PURE__ */ from_html(`<span class="new svelte-i436af">new</span>`);
var root_8$9 = /* @__PURE__ */ from_html(`<div role="option" tabindex="-1"><span class="ic svelte-i436af"><!></span> <span class="title svelte-i436af"> </span> <!> <span class="meta svelte-i436af"> </span> <span class="age svelte-i436af"> </span></div>`);
var root_9$7 = /* @__PURE__ */ from_html(`<div class="folded svelte-i436af"><!> <span class="svelte-i436af"> </span></div>`);
var root_10$7 = /* @__PURE__ */ from_html(`<div class="list svelte-i436af"><header class="svelte-i436af"><span class="t svelte-i436af">What needs you</span> <span class="n svelte-i436af"> </span></header> <!> <!> <div class="rows svelte-i436af" role="listbox" aria-label="what needs you" tabindex="0"><!> <!> <!> <!></div></div>`);
function List$1($$anchor, $$props) {
	push($$props, true);
	let items = prop($$props, "items", 19, () => []), folded = prop($$props, "folded", 19, () => []), inhibited = prop($$props, "inhibited", 19, () => []), close = prop($$props, "close", 3, null), project = prop($$props, "project", 3, ""), focus = prop($$props, "focus", 3, ""), loaded = prop($$props, "loaded", 3, false), error = prop($$props, "error", 3, null);
	const narrow = narrowed(() => project());
	const projects = /* @__PURE__ */ user_derived(() => [...new Set(items().map((i) => i.project_name).filter((p) => !!p))]);
	const NONE = {
		items: [],
		folded: [],
		inhibited: []
	};
	const narrowedFeed = /* @__PURE__ */ user_derived(() => narrow.on ? narrow.data ?? NONE : null);
	const shown = /* @__PURE__ */ user_derived(() => get(narrowedFeed)?.items ?? items());
	const shownFolded = /* @__PURE__ */ user_derived(() => get(narrowedFeed)?.folded ?? folded());
	const shownInhibited = /* @__PURE__ */ user_derived(() => get(narrowedFeed)?.inhibited ?? inhibited());
	const ready = /* @__PURE__ */ user_derived(() => narrow.on ? narrow.phase !== "loading" : loaded());
	const nothingRaised = /* @__PURE__ */ user_derived(() => get(shown).length === 0 && get(shownFolded).length === 0 && get(shownInhibited).length === 0);
	const missing = /* @__PURE__ */ user_derived(() => narrow.missing);
	const selected = /* @__PURE__ */ user_derived(() => place(get(shown), address(focus(), items())).current?.id ?? "");
	const optionId = (id) => `inbox-row-${id.replace(/[^\w-]/g, "_")}`;
	const icon = (k) => k === "permission" ? "shield" : k === "question" ? "question" : k.includes("gate") || k.includes("fail") ? "x" : k.includes("ready") ? "check" : k.includes("conflict") ? "alert" : k.includes("context") ? "clock" : "dot";
	let now = /* @__PURE__ */ state(proxy(Date.now()));
	user_effect(() => {
		const t = setInterval(() => set(now, Date.now(), true), 3e4);
		return () => clearInterval(t);
	});
	const age = (since) => since ? ago$1(Math.max(0, (get(now) - Date.parse(since)) / 1e3)) : "";
	var div = root_10$7();
	var header = child(div);
	var text = only_child(sibling(child(header), 2), true);
	reset(header);
	var node = sibling(header, 2);
	var consequent = ($$anchor) => {
		var p_1 = root$18();
		var node_1 = child(p_1);
		Icon(node_1, {
			name: "clock",
			size: 12
		});
		var text_1 = sibling(node_1);
		reset(p_1);
		template_effect(() => set_text(text_1, ` since you last looked · ${close().since_last_look ?? ""}`));
		append($$anchor, p_1);
	};
	if_block(node, ($$render) => {
		if (close()?.since_last_look) $$render(consequent);
	});
	var node_2 = sibling(node, 2);
	var consequent_1 = ($$anchor) => {
		var div_1 = root_2$15();
		var button = child(div_1);
		let classes;
		each(sibling(button, 2), 16, () => get(projects), (p) => p, ($$anchor, p) => {
			var button_1 = root_1$18();
			let classes_1;
			var text_2 = only_child(button_1, true);
			template_effect(() => {
				set_attribute(button_1, "aria-pressed", project() === p);
				classes_1 = set_class(button_1, 1, "svelte-i436af", null, classes_1, { on: project() === p });
				set_text(text_2, p);
			});
			delegated("click", button_1, () => $$props.open(project() === p ? "" : p));
			append($$anchor, button_1);
		});
		reset(div_1);
		template_effect(() => {
			set_attribute(button, "aria-pressed", !project());
			classes = set_class(button, 1, "svelte-i436af", null, classes, { on: !project() });
		});
		delegated("click", button, () => $$props.open(""));
		append($$anchor, div_1);
	};
	if_block(node_2, ($$render) => {
		if (get(projects).length > 1 || project()) $$render(consequent_1);
	});
	var div_2 = sibling(node_2, 2);
	var node_4 = child(div_2);
	var consequent_2 = ($$anchor) => {
		var p_2 = root_3$13();
		var text_3 = only_child(p_2);
		template_effect(() => set_text(text_3, `Could not read the inbox narrowed to ${project() ?? ""}: ${narrow.failure.says ?? ""}.`));
		append($$anchor, p_2);
	};
	var consequent_3 = ($$anchor) => {
		var p_3 = root_3$13();
		var text_4 = only_child(p_3);
		template_effect(() => set_text(text_4, `No project is named “${project() ?? ""}”.`));
		append($$anchor, p_3);
	};
	var consequent_4 = ($$anchor) => {
		var fragment = comment();
		each(first_child(fragment), 16, () => [
			0,
			1,
			2
		], (i) => i, ($$anchor, i) => {
			append($$anchor, root_4$12());
		});
		append($$anchor, fragment);
	};
	var consequent_5 = ($$anchor) => {
		append($$anchor, root_5$10());
	};
	var consequent_6 = ($$anchor) => {
		var p_5 = root_6$9();
		var text_5 = only_child(p_5, true);
		template_effect(() => set_text(text_5, project() ? `Nothing needs you in ${project()}.` : "Nothing needs you."));
		append($$anchor, p_5);
	};
	if_block(node_4, ($$render) => {
		if (narrow.on && narrow.failure && !narrow.data) $$render(consequent_2);
		else if (get(missing)) $$render(consequent_3, 1);
		else if (!get(ready) && get(shown).length === 0) $$render(consequent_4, 2);
		else if (get(nothingRaised) && error()) $$render(consequent_5, 3);
		else if (get(nothingRaised)) $$render(consequent_6, 4);
	});
	var node_6 = sibling(node_4, 2);
	each(node_6, 17, () => get(shown), (i) => i.id, ($$anchor, i) => {
		var div_4 = root_8$9();
		let classes_2;
		var span_1 = child(div_4);
		var node_7 = child(span_1);
		{
			let $0 = /* @__PURE__ */ user_derived(() => icon(get(i).kind));
			Icon(node_7, {
				get name() {
					return get($0);
				},
				size: 14
			});
		}
		reset(span_1);
		var span_2 = sibling(span_1, 2);
		var text_6 = only_child(span_2, true);
		var node_8 = sibling(span_2, 2);
		var consequent_7 = ($$anchor) => {
			append($$anchor, root_7$9());
		};
		if_block(node_8, ($$render) => {
			if (get(i).new_to_you) $$render(consequent_7);
		});
		var span_4 = sibling(node_8, 2);
		var text_7 = only_child(span_4);
		var text_8 = only_child(sibling(span_4, 2), true);
		reset(div_4);
		template_effect(($0, $1, $2) => {
			classes_2 = set_class(div_4, 1, "row svelte-i436af", null, classes_2, { high: get(i).level === "high" });
			set_attribute(div_4, "id", $0);
			set_attribute(div_4, "aria-selected", get(i).id === get(selected));
			set_text(text_6, get(i).title);
			set_text(text_7, `${$1 ?? ""}${get(i).project_name ? ` · ${get(i).project_name}` : ""}`);
			set_text(text_8, $2);
		}, [
			() => optionId(get(i).id),
			() => get(i).kind.replace(/_/g, " "),
			() => age(get(i).since)
		]);
		delegated("click", div_4, () => $$props.open(itemFocus(get(i).id)));
		delegated("keydown", div_4, (e) => {
			if (e.key === "Enter" || e.key === " ") {
				e.preventDefault();
				$$props.open(itemFocus(get(i).id));
			}
		});
		append($$anchor, div_4);
	});
	var node_9 = sibling(node_6, 2);
	each(node_9, 17, () => get(shownFolded), (f) => f.kind + (f.project ?? ""), ($$anchor, f) => {
		var div_5 = root_9$7();
		var node_10 = child(div_5);
		Icon(node_10, {
			name: "more",
			size: 13
		});
		var text_9 = sibling(node_10);
		var text_10 = only_child(sibling(text_9));
		reset(div_5);
		template_effect(($0) => {
			set_text(text_9, ` ${get(f).count ?? ""} × ${$0 ?? ""} `);
			set_text(text_10, `${get(f).project ?? "across projects" ?? ""} · folded`);
		}, [() => get(f).kind.replace(/_/g, " ")]);
		append($$anchor, div_5);
	});
	each(sibling(node_9, 2), 17, () => get(shownInhibited), (s) => s.cause, ($$anchor, s) => {
		var div_6 = root_9$7();
		var node_12 = child(div_6);
		Icon(node_12, {
			name: "more",
			size: 13
		});
		var text_11 = sibling(node_12);
		var text_12 = only_child(sibling(text_11), true);
		reset(div_6);
		template_effect(() => {
			set_text(text_11, ` ${get(s).count ?? ""} more counted `);
			set_text(text_12, get(s).because);
		});
		append($$anchor, div_6);
	});
	reset(div_2);
	reset(div);
	template_effect(($0) => {
		set_text(text, get(ready) && !get(missing) && !(narrow.on && narrow.failure && !narrow.data) ? get(shown).length : "");
		set_attribute(div_2, "aria-activedescendant", $0);
	}, [() => get(selected) ? optionId(get(selected)) : void 0]);
	append($$anchor, div);
	pop();
}
delegate(["click", "keydown"]);
//#endregion
//#region src/surfaces/inbox/index.ts
bindList("inbox", "move into the item (Tab then reaches its controls)");
bind({
	surface: "inbox",
	combo: "a",
	action: "inbox-allow",
	label: "allow the permission on screen"
});
bind({
	surface: "inbox",
	combo: "d",
	action: "inbox-deny",
	label: "deny the permission on screen"
});
bind({
	surface: "global",
	combo: "g i",
	action: "go-inbox",
	label: "go to the inbox"
});
onAction("go-inbox", () => {
	go("#inbox");
	return true;
});
register({
	id: "inbox",
	icon: "inbox",
	title: "Inbox",
	heading: "What needs you",
	band: "attention",
	order: 0,
	marksLook: true,
	status: (feed) => {
		const i = feed.inbox;
		if (!i?.items) return [];
		const s = feed.board?.summary;
		return [{
			n: i.items.filter((x) => !!x.ask || (x.options ?? []).length > 0 || (x.actions ?? []).some((a) => a !== "open" && a !== "snooze")).length,
			word: "need you",
			icon: "inbox",
			tone: "wait",
			end: true
		}, ...s?.asks_waiting ? [{
			n: s.asks_waiting,
			word: "asked, session gone",
			icon: "question",
			tone: "wait",
			end: true
		}] : []];
	},
	count: (feed) => {
		return feed.inbox?.items?.length ?? null;
	},
	link: "ask",
	linkFocus: (id) => `ask=${id}`,
	select: (feed, focus) => {
		const b = feed.inbox;
		const items = b?.items ?? [];
		const { item, wanted, project } = address(focus ?? "", items);
		const watching = feed.board?.watching ?? null;
		return {
			items,
			folded: b?.folded ?? [],
			inhibited: b?.inhibited ?? [],
			close: b?.close ?? null,
			project,
			item,
			wanted,
			watching,
			...phase(feed)
		};
	},
	tab: (feed, focus) => {
		const items = feed.inbox?.items ?? [];
		const { item, project } = address(focus, items);
		return items.find((i) => i.id === item)?.title ?? (project ? `Inbox: ${project}` : "Inbox");
	},
	side: List$1,
	component: Inbox
});
//#endregion
//#region src/surfaces/new/New.svelte
function startable(targets, checking, failed) {
	return !!targets && targets.length > 0 && targets.every((t) => !t.refusal) && !checking && !failed;
}
var root$17 = /* @__PURE__ */ from_html(`<input class="filter svelte-wpxmn" placeholder="Filter projects" aria-label="filter projects"/>`);
var root_1$17 = /* @__PURE__ */ from_html(`<span class="untrusted svelte-wpxmn">not trusted</span>`);
var root_2$14 = /* @__PURE__ */ from_html(`<button type="button"><!> <!></button>`);
var root_3$12 = /* @__PURE__ */ from_html(`<p class="quiet svelte-wpxmn">Reading the projects…</p>`);
var root_4$11 = /* @__PURE__ */ from_html(`<p class="quiet svelte-wpxmn">No project is registered yet. <code class="svelte-wpxmn">devplane trust &lt;path&gt;</code> adds one.</p>`);
var root_5$9 = /* @__PURE__ */ from_html(`<option> </option>`);
var root_6$8 = /* @__PURE__ */ from_html(`<section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="spec">Specification <span class="hint svelte-wpxmn">optional — the change works to it, and its tasks are traced</span></label> <select id="spec" class="svelte-wpxmn"><option>none — just the words below</option><!></select></section>`);
var root_7$8 = /* @__PURE__ */ from_html(`<p class="quiet svelte-wpxmn">Pick where and give it a title, and what starting it would do is shown here.</p>`);
var root_8$8 = /* @__PURE__ */ from_html(`<p class="fail svelte-wpxmn"><!> </p>`);
var root_9$6 = /* @__PURE__ */ from_html(`<p class="quiet svelte-wpxmn">Checking…</p>`);
var root_10$6 = /* @__PURE__ */ from_html(`<span class="note svelte-wpxmn"> </span>`);
var root_11$4 = /* @__PURE__ */ from_html(`<li><!> <b class="svelte-wpxmn"> </b> <span class="svelte-wpxmn"> </span> <!></li>`);
var root_12$2 = /* @__PURE__ */ from_html(`<ul class="svelte-wpxmn"></ul>`);
var root_13$2 = /* @__PURE__ */ from_html(`<p class="fail svelte-wpxmn"> </p>`);
var root_14$2 = /* @__PURE__ */ from_html(`<form class="new svelte-wpxmn" aria-label="start a new change"><header class="svelte-wpxmn"><!> <h1 class="svelte-wpxmn">New change</h1> <button type="button" class="x svelte-wpxmn" aria-label="close"><!></button></header> <div class="body svelte-wpxmn"><section class="svelte-wpxmn"><span class="label svelte-wpxmn">Where <span class="hint svelte-wpxmn">one project, or the same change in several</span></span> <!> <div class="projects svelte-wpxmn" role="group" aria-label="projects"></div></section> <!> <!> <section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="title">Title <span class="hint svelte-wpxmn">also the branch name</span></label> <input id="title" placeholder="Rate-limit the login route" class="svelte-wpxmn"/></section> <section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="prompt">What to do <span class="hint svelte-wpxmn">the agent's first prompt; the title is used when empty</span></label> <textarea id="prompt" rows="5" placeholder="Five failed logins a minute per account; the sixth is rejected with Retry-After." class="svelte-wpxmn"></textarea></section> <div class="row svelte-wpxmn"><section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="agent">Agent</label> <select id="agent" class="svelte-wpxmn"><option> </option><!></select></section> <section class="svelte-wpxmn"><span class="label svelte-wpxmn">Isolation</span> <label class="check svelte-wpxmn"><input type="checkbox" class="svelte-wpxmn"/> Its own worktree <span class="hint svelte-wpxmn"> </span></label></section></div> <section class="preflight svelte-wpxmn" aria-live="polite"><span class="label svelte-wpxmn">Before anything is created</span> <!></section> <!></div> <footer class="svelte-wpxmn"><span class="quiet svelte-wpxmn"> </span> <button type="button" class="svelte-wpxmn">Cancel</button> <button type="submit" class="primary svelte-wpxmn"><!> </button></footer></form>`);
function New($$anchor, $$props) {
	push($$props, true);
	let changeSurface = prop($$props, "changeSurface", 3, "change"), planted = prop($$props, "targets", 3, null);
	const projectsRead = resource(() => "/api/projects", { tell: () => "devplane doctor" });
	const agentsRead = resource(() => "/api/agents", { tell: () => "devplane agents" });
	const specsRead = resource(() => "/api/specs", { tell: () => "devplane doctor" });
	const projects = /* @__PURE__ */ user_derived(() => Array.isArray(projectsRead.data) ? projectsRead.data : []);
	const agents = /* @__PURE__ */ user_derived(() => Array.isArray(agentsRead.data) ? agentsRead.data : []);
	const specs = /* @__PURE__ */ user_derived(() => specsRead.data?.projects ?? []);
	let chosen = /* @__PURE__ */ state(proxy([]));
	let title = /* @__PURE__ */ state("");
	let prompt = /* @__PURE__ */ state("");
	let agent = /* @__PURE__ */ state("");
	let spec = /* @__PURE__ */ state("");
	let worktree = /* @__PURE__ */ state(true);
	let filter = /* @__PURE__ */ state("");
	const shownProjects = /* @__PURE__ */ user_derived(() => get(projects).filter((p) => !get(filter).trim() || p.name.toLowerCase().includes(get(filter).trim().toLowerCase())));
	const one = /* @__PURE__ */ user_derived(() => get(chosen).length === 1 ? get(projects).find((p) => p.id === get(chosen)[0]) : null);
	const plans = /* @__PURE__ */ user_derived(() => get(one) ? get(specs).find((s) => s.root === get(one).root || s.project_id === get(one).id)?.plans ?? [] : []);
	function toggle(id) {
		set(chosen, get(chosen).includes(id) ? get(chosen).filter((x) => x !== id) : [...get(chosen), id], true);
		if (get(chosen).length !== 1) set(spec, "");
	}
	const body = /* @__PURE__ */ user_derived(() => ({
		projects: get(chosen),
		title: get(title).trim(),
		prompt: get(prompt).trim() || null,
		agent: get(agent) || null,
		spec: get(spec) || null,
		worktree: get(worktree)
	}));
	let fetchedTargets = /* @__PURE__ */ state(null);
	const targets = /* @__PURE__ */ user_derived(() => get(fetchedTargets) ?? planted());
	let checking = /* @__PURE__ */ state(false);
	let checkError = /* @__PURE__ */ state("");
	user_effect(() => {
		const b = get(body);
		if (b.projects.length === 0 || !b.title) {
			set(fetchedTargets, null);
			set(checkError, "");
			return;
		}
		set(checking, true);
		let live = true;
		const t = setTimeout(() => {
			api("/api/changes/preflight", {
				method: "POST",
				body: JSON.stringify(b)
			}).then((r) => {
				if (live) {
					set(fetchedTargets, r.targets ?? [], true);
					set(checkError, "");
				}
			}).catch((e) => {
				if (live) set(checkError, e instanceof Error ? e.message : String(e), true);
			}).finally(() => {
				if (live) set(checking, false);
			});
		}, 350);
		return () => {
			live = false;
			clearTimeout(t);
		};
	});
	const refused = /* @__PURE__ */ user_derived(() => (get(targets) ?? []).filter((t) => t.refusal));
	const ready = /* @__PURE__ */ user_derived(() => startable(get(targets), get(checking), get(checkError)));
	const starting = writer();
	let said = /* @__PURE__ */ state("");
	async function start(e) {
		e.preventDefault();
		if (!get(ready) || starting.busy) return;
		await starting.run("start", async () => {
			try {
				const r = await api("/api/changes", {
					method: "POST",
					body: JSON.stringify(get(body))
				});
				const first = r.change_id ?? r.changes?.[0]?.change_id;
				if (r.error) set(said, r.error, true);
				if (first) go(`#${changeSurface()}/${encodeURIComponent(first)}`);
			} catch (err) {
				const f = failure(err, "devplane change list");
				set(said, `Nothing was started: ${f.says}. \`${f.tell}\` tells more.`);
			}
		});
	}
	const close = () => run("leave", "new");
	var form = root_14$2();
	var header = child(form);
	var node = child(header);
	Icon(node, {
		name: "change",
		size: 16
	});
	var button = sibling(node, 4);
	Icon(child(button), {
		name: "x",
		size: 14
	});
	reset(button);
	reset(header);
	var div = sibling(header, 2);
	var section = child(div);
	var node_2 = sibling(child(section), 2);
	var consequent = ($$anchor) => {
		var input = root$17();
		remove_input_defaults(input);
		bind_value(input, () => get(filter), ($$value) => set(filter, $$value));
		append($$anchor, input);
	};
	if_block(node_2, ($$render) => {
		if (get(projects).length > 6) $$render(consequent);
	});
	var div_1 = sibling(node_2, 2);
	each(div_1, 21, () => get(shownProjects), (p) => p.id, ($$anchor, p) => {
		var button_1 = root_2$14();
		let classes;
		var node_3 = child(button_1);
		{
			let $0 = /* @__PURE__ */ user_derived(() => get(chosen).includes(get(p).id) ? "check" : "folder");
			Icon(node_3, {
				get name() {
					return get($0);
				},
				size: 13
			});
		}
		var text = sibling(node_3);
		var node_4 = sibling(text);
		var consequent_1 = ($$anchor) => {
			append($$anchor, root_1$17());
		};
		if_block(node_4, ($$render) => {
			if (!get(p).trusted) $$render(consequent_1);
		});
		reset(button_1);
		template_effect(($0, $1) => {
			classes = set_class(button_1, 1, "proj svelte-wpxmn", null, classes, { on: $0 });
			set_attribute(button_1, "aria-pressed", $1);
			set_text(text, ` ${get(p).name ?? ""} `);
		}, [() => get(chosen).includes(get(p).id), () => get(chosen).includes(get(p).id)]);
		delegated("click", button_1, () => toggle(get(p).id));
		append($$anchor, button_1);
	}, ($$anchor) => {
		var fragment = comment();
		var node_5 = first_child(fragment);
		var consequent_2 = ($$anchor) => {
			Failed($$anchor, {
				what: "the projects",
				get failure() {
					return projectsRead.failure;
				}
			});
		};
		var consequent_3 = ($$anchor) => {
			append($$anchor, root_3$12());
		};
		var alternate = ($$anchor) => {
			append($$anchor, root_4$11());
		};
		if_block(node_5, ($$render) => {
			if (projectsRead.phase === "failed" && projectsRead.failure) $$render(consequent_2);
			else if (projectsRead.phase === "loading") $$render(consequent_3, 1);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment);
	});
	reset(div_1);
	reset(section);
	var node_6 = sibling(section, 2);
	var consequent_4 = ($$anchor) => {
		Failed($$anchor, {
			what: "the specifications, so none can be picked",
			get failure() {
				return specsRead.failure;
			}
		});
	};
	if_block(node_6, ($$render) => {
		if (get(one) && specsRead.failure) $$render(consequent_4);
	});
	var node_7 = sibling(node_6, 2);
	var consequent_5 = ($$anchor) => {
		var section_1 = root_6$8();
		var select = sibling(child(section_1), 2);
		var option = child(select);
		option.value = option.__value = "";
		each(sibling(option), 17, () => get(plans), (p) => p.plan.path, ($$anchor, p) => {
			var option_1 = root_5$9();
			var text_1 = only_child(option_1);
			var option_1_value = {};
			template_effect(() => {
				option_1.disabled = !!get(p).change_id;
				set_text(text_1, `${get(p).plan.path ?? ""}${get(p).plan.progress ? ` · ${get(p).plan.progress.done} ticked · ${get(p).plan.progress.total} tasks` : ""}${get(p).change_id ? " · a change already works to it" : ""}`);
				if (option_1_value !== (option_1_value = get(p).plan.path)) option_1.value = (option_1.__value = option_1_value) ?? "";
			});
			append($$anchor, option_1);
		});
		reset(select);
		init_select(select);
		reset(section_1);
		bind_select_value(select, () => get(spec), ($$value) => set(spec, $$value));
		append($$anchor, section_1);
	};
	if_block(node_7, ($$render) => {
		if (get(one) && get(plans).length > 0) $$render(consequent_5);
	});
	var section_2 = sibling(node_7, 2);
	var input_1 = sibling(child(section_2), 2);
	remove_input_defaults(input_1);
	autofocus(input_1, true);
	reset(section_2);
	var section_3 = sibling(section_2, 2);
	var textarea = sibling(child(section_3), 2);
	remove_textarea_child(textarea);
	reset(section_3);
	var div_2 = sibling(section_3, 2);
	var section_4 = child(div_2);
	var select_1 = sibling(child(section_4), 2);
	var option_2 = child(select_1);
	var text_2 = only_child(option_2, true);
	option_2.value = option_2.__value = "";
	each(sibling(option_2), 17, () => get(agents), (a) => a.id, ($$anchor, a) => {
		var option_3 = root_5$9();
		var text_3 = only_child(option_3, true);
		var option_3_value = {};
		template_effect(() => {
			set_text(text_3, get(a).name ?? get(a).id);
			if (option_3_value !== (option_3_value = get(a).id)) option_3.value = (option_3.__value = option_3_value) ?? "";
		});
		append($$anchor, option_3);
	});
	reset(select_1);
	init_select(select_1);
	reset(section_4);
	var section_5 = sibling(section_4, 2);
	var label = sibling(child(section_5), 2);
	var input_2 = child(label);
	remove_input_defaults(input_2);
	var text_4 = only_child(sibling(input_2, 2), true);
	reset(label);
	reset(section_5);
	reset(div_2);
	var section_6 = sibling(div_2, 2);
	var node_10 = sibling(child(section_6), 2);
	var consequent_6 = ($$anchor) => {
		append($$anchor, root_7$8());
	};
	var d = /* @__PURE__ */ user_derived(() => !get(targets) && (get(chosen).length === 0 || !get(title).trim()));
	var consequent_7 = ($$anchor) => {
		var p_4 = root_8$8();
		var node_11 = child(p_4);
		Icon(node_11, {
			name: "alert",
			size: 13
		});
		var text_5 = sibling(node_11);
		reset(p_4);
		template_effect(() => set_text(text_5, ` ${get(checkError) ?? ""}`));
		append($$anchor, p_4);
	};
	var consequent_8 = ($$anchor) => {
		append($$anchor, root_9$6());
	};
	var alternate_1 = ($$anchor) => {
		var ul = root_12$2();
		each(ul, 21, () => get(targets), (t) => t.asked, ($$anchor, t) => {
			var li = root_11$4();
			let classes_1;
			var node_12 = child(li);
			{
				let $0 = /* @__PURE__ */ user_derived(() => get(t).refusal ? "x" : "check");
				Icon(node_12, {
					get name() {
						return get($0);
					},
					size: 14
				});
			}
			var b_1 = sibling(node_12, 2);
			var text_6 = only_child(b_1, true);
			var span_2 = sibling(b_1, 2);
			var text_7 = only_child(span_2, true);
			each(sibling(span_2, 2), 17, () => get(t).notes, index, ($$anchor, n) => {
				var span_3 = root_10$6();
				var text_8 = only_child(span_3, true);
				template_effect(() => set_text(text_8, get(n)));
				append($$anchor, span_3);
			});
			reset(li);
			template_effect(() => {
				classes_1 = set_class(li, 1, "svelte-wpxmn", null, classes_1, { bad: !!get(t).refusal });
				set_text(text_6, get(t).name);
				set_text(text_7, get(t).says);
			});
			append($$anchor, li);
		});
		reset(ul);
		append($$anchor, ul);
	};
	if_block(node_10, ($$render) => {
		if (get(d)) $$render(consequent_6);
		else if (get(checkError)) $$render(consequent_7, 1);
		else if (!get(targets)) $$render(consequent_8, 2);
		else $$render(alternate_1, -1);
	});
	reset(section_6);
	var node_14 = sibling(section_6, 2);
	var consequent_9 = ($$anchor) => {
		var p_6 = root_13$2();
		var text_9 = only_child(p_6, true);
		template_effect(() => set_text(text_9, get(said)));
		append($$anchor, p_6);
	};
	if_block(node_14, ($$render) => {
		if (get(said)) $$render(consequent_9);
	});
	reset(div);
	var footer = sibling(div, 2);
	var span_4 = child(footer);
	var text_10 = only_child(span_4, true);
	var button_2 = sibling(span_4, 2);
	var button_3 = sibling(button_2, 2);
	var node_15 = child(button_3);
	Icon(node_15, {
		name: "play",
		size: 13
	});
	var text_11 = sibling(node_15);
	reset(button_3);
	reset(footer);
	reset(form);
	template_effect(() => {
		set_text(text_2, agentsRead.failure ? "the project's default (the agents could not be read)" : "the project's default");
		set_text(text_4, get(worktree) ? "your checkout is never touched" : "in place — no parallel safety, and the diff is against your working tree");
		set_text(text_10, get(refused).length ? `${get(refused).length} refused — nothing will be created until every project can start` : get(chosen).length > 1 ? `${get(chosen).length} changes, one per project` : "");
		button_3.disabled = !get(ready) || !!starting.busy;
		set_text(text_11, ` ${starting.busy ? `Starting · ${starting.elapsed}` : get(chosen).length > 1 ? `Start ${get(chosen).length} changes` : "Start the change"}`);
	});
	event("submit", form, start);
	delegated("click", button, close);
	bind_value(input_1, () => get(title), ($$value) => set(title, $$value));
	bind_value(textarea, () => get(prompt), ($$value) => set(prompt, $$value));
	bind_select_value(select_1, () => get(agent), ($$value) => set(agent, $$value));
	bind_checked(input_2, () => get(worktree), ($$value) => set(worktree, $$value));
	delegated("click", button_2, close);
	append($$anchor, form);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/new/index.ts
onAction("new-change", () => {
	if (location.hash.startsWith("#new")) return true;
	try {
		sessionStorage.setItem("vp-before-new", location.hash);
	} catch {}
	go("#new");
	return true;
});
onAction("leave", (surface) => {
	if (surface !== "new") return false;
	let back = "";
	try {
		back = sessionStorage.getItem("vp-before-new") ?? "";
	} catch {}
	go(back);
	return true;
});
register({
	id: "new",
	title: "New change",
	heading: "New change",
	band: "steering",
	order: 0,
	nav: false,
	transient: true,
	reads: [
		"/api/projects",
		"/api/agents",
		"/api/specs",
		"/api/changes/preflight"
	],
	select: () => ({ changeSurface: surfaces().find((s) => s.link === "change")?.id ?? "" }),
	component: New
});
//#endregion
//#region src/surfaces/palette/Palette.svelte
function score(q, text) {
	const t = text.toLowerCase();
	if (!q) return 0;
	const at = t.indexOf(q);
	if (at === 0) return 0;
	if (at > 0) return /\W/.test(t[at - 1]) ? 1 : 2;
	let i = 0;
	let gaps = 0;
	let last = -1;
	for (const ch of q) {
		const j = t.indexOf(ch, i);
		if (j === -1) return null;
		if (last !== -1 && j !== last + 1) gaps++;
		last = j;
		i = j + 1;
	}
	return 3 + gaps;
}
var ORDER = [
	"change",
	"surface",
	"action",
	"project"
];
function rank(entries, raw) {
	const typed = raw.trim().toLowerCase();
	const actions = typed.startsWith(">");
	const q = actions ? typed.slice(1).trim() : typed;
	return entries.filter((e) => !actions || e.kind === "action").map((e) => ({
		e,
		s: score(q, e.label)
	})).filter((x) => x.s !== null).sort((a, b) => ORDER.indexOf(a.e.kind) - ORDER.indexOf(b.e.kind) || a.s - b.s).map((x) => x.e);
}
var root$16 = /* @__PURE__ */ from_html(`<div class="unread svelte-d7fgwz"><!></div>`);
var root_1$16 = /* @__PURE__ */ from_html(`<li class="sec svelte-d7fgwz" role="presentation"> </li>`);
var root_2$13 = /* @__PURE__ */ from_html(`<span> </span>`);
var root_3$11 = /* @__PURE__ */ from_html(`<!> <li role="option" class="svelte-d7fgwz"><button tabindex="-1" class="svelte-d7fgwz"><!> <span class="label svelte-d7fgwz"> </span> <!><!></button></li>`, 1);
var root_4$10 = /* @__PURE__ */ from_html(`<li class="none svelte-d7fgwz"> </li>`);
var root_5$8 = /* @__PURE__ */ from_html(`<section class="palette svelte-d7fgwz" aria-labelledby="palette-head"><h2 id="palette-head" class="sr-only">Everything, by name</h2> <label class="field svelte-d7fgwz"><!> <input placeholder="Type a change, a place, an action — or > for actions only" aria-label="find by name" role="combobox" aria-expanded="true" aria-controls="palette-matches" aria-autocomplete="list" class="svelte-d7fgwz"/></label> <!> <!> <ul role="listbox" aria-label="matches" id="palette-matches" class="svelte-d7fgwz"><!> <!></ul> <footer class="svelte-d7fgwz"><span><kbd class="svelte-d7fgwz">↑</kbd><kbd class="svelte-d7fgwz">↓</kbd> move</span><span><kbd class="svelte-d7fgwz">↵</kbd> open</span><span><kbd class="svelte-d7fgwz">esc</kbd> close</span></footer></section>`);
function Palette($$anchor, $$props) {
	push($$props, true);
	let from = prop($$props, "from", 3, ""), changes = prop($$props, "changes", 3, null), projects = prop($$props, "projects", 3, null);
	let query = /* @__PURE__ */ state("");
	const changesRead = resource(() => "/api/changes", { tell: () => "devplane change list" });
	const projectsRead = resource(() => "/api/projects", { tell: () => "devplane doctor" });
	const fetchedChanges = /* @__PURE__ */ user_derived(() => Array.isArray(changesRead.data) ? changesRead.data : null);
	const fetchedProjects = /* @__PURE__ */ user_derived(() => Array.isArray(projectsRead.data) ? projectsRead.data : null);
	const LIST_ACTIONS = /* @__PURE__ */ new Set([
		"next",
		"prev",
		"first",
		"last",
		"open"
	]);
	const over = /* @__PURE__ */ user_derived(() => from().replace(/^#/, "").split("/")[0] || landing()?.id || "");
	const opensChange = /* @__PURE__ */ user_derived(() => surfaces().find((s) => s.link === "change"));
	const entries = /* @__PURE__ */ user_derived(() => {
		const out = [];
		for (const c of get(fetchedChanges) ?? changes() ?? []) {
			const to = get(opensChange)?.id;
			if (!to) continue;
			out.push({
				kind: "change",
				icon: "change",
				label: c.title || c.id,
				hint: c.state,
				qualifier: c.qualifier,
				go: () => go(`#${to}/${encodeURIComponent(c.id)}`)
			});
		}
		for (const s of listed$1()) out.push({
			kind: "surface",
			icon: s.icon ?? "right",
			label: `Go to ${s.title}`,
			go: () => go(`#${s.id}`)
		});
		for (const b of help(get(over))) {
			if (b.action === "open-palette" || b.action === "leave" || LIST_ACTIONS.has(b.action)) continue;
			out.push({
				kind: "action",
				icon: "keyboard",
				label: b.label.charAt(0).toUpperCase() + b.label.slice(1),
				hint: spell(b.combo),
				go: () => {
					go(from() || "");
					setTimeout(() => run(b.action, get(over)), 0);
				}
			});
		}
		for (const p of get(fetchedProjects) ?? projects() ?? []) {
			const to = landing()?.id;
			if (!to) continue;
			out.push({
				kind: "project",
				icon: "folder",
				label: p.name,
				hint: "narrow to this project",
				go: () => go(`#${to}/${encodeURIComponent(p.name)}`)
			});
		}
		return out;
	});
	const TITLES = {
		change: "Changes",
		surface: "Go to",
		action: "Actions",
		project: "Projects"
	};
	const searcher = /* @__PURE__ */ user_derived(() => surfaces().find((s) => s.takesQuery));
	const shown = /* @__PURE__ */ user_derived(() => {
		const raw = get(query).trim();
		const actions = raw.startsWith(">");
		return rank(get(entries), raw).concat(raw && !actions && get(searcher) ? [{
			kind: "surface",
			icon: "search",
			label: `Search every session for “${raw}”`,
			go: () => go(`#${get(searcher).id}/${encodeURIComponent(raw)}`)
		}] : []);
	});
	let at = /* @__PURE__ */ state(0);
	user_effect(() => {
		get(query);
		set(at, 0);
	});
	function typed(e) {
		if (e.key === "ArrowDown" || e.ctrlKey && e.key === "n") {
			set(at, Math.min(get(shown).length - 1, get(at) + 1), true);
			e.preventDefault();
		} else if (e.key === "ArrowUp" || e.ctrlKey && e.key === "p") {
			set(at, Math.max(0, get(at) - 1), true);
			e.preventDefault();
		} else if (e.key === "Enter") {
			get(shown)[get(at)]?.go();
			e.preventDefault();
		}
		queueMicrotask(() => document.querySelector(".palette li[aria-selected='true']")?.scrollIntoView({ block: "nearest" }));
	}
	const bound = /* @__PURE__ */ user_derived(() => all().length);
	var section = root_5$8();
	var label = sibling(child(section), 2);
	var node = child(label);
	Icon(node, {
		name: "search",
		size: 16
	});
	var input = sibling(node, 2);
	remove_input_defaults(input);
	autofocus(input, true);
	reset(label);
	var node_1 = sibling(label, 2);
	var consequent = ($$anchor) => {
		var div = root$16();
		Failed(child(div), {
			what: "the changes, so none are listed",
			get failure() {
				return changesRead.failure;
			}
		});
		reset(div);
		append($$anchor, div);
	};
	if_block(node_1, ($$render) => {
		if (changesRead.failure && !changesRead.data) $$render(consequent);
	});
	var node_3 = sibling(node_1, 2);
	var consequent_1 = ($$anchor) => {
		var div_1 = root$16();
		Failed(child(div_1), {
			what: "the projects, so none are listed",
			get failure() {
				return projectsRead.failure;
			}
		});
		reset(div_1);
		append($$anchor, div_1);
	};
	if_block(node_3, ($$render) => {
		if (projectsRead.failure && !projectsRead.data) $$render(consequent_1);
	});
	var ul = sibling(node_3, 2);
	var node_5 = child(ul);
	each(node_5, 17, () => get(shown), index, ($$anchor, e, i) => {
		var fragment = root_3$11();
		var node_6 = first_child(fragment);
		var consequent_2 = ($$anchor) => {
			var li = root_1$16();
			var text_1 = only_child(li, true);
			template_effect(() => set_text(text_1, TITLES[get(e).kind]));
			append($$anchor, li);
		};
		if_block(node_6, ($$render) => {
			if (i === 0 || get(shown)[i - 1].kind !== get(e).kind) $$render(consequent_2);
		});
		var li_1 = sibling(node_6, 2);
		set_attribute(li_1, "id", `palette-opt-${i}`);
		var button = child(li_1);
		var node_7 = child(button);
		Icon(node_7, {
			get name() {
				return get(e).icon;
			},
			size: 14
		});
		var span = sibling(node_7, 2);
		var text_2 = only_child(span, true);
		var node_8 = sibling(span, 2);
		var consequent_3 = ($$anchor) => {
			var span_1 = root_2$13();
			let classes;
			var text_3 = only_child(span_1, true);
			template_effect(() => {
				classes = set_class(span_1, 1, "hint svelte-d7fgwz", null, classes, { kbd: get(e).kind === "action" });
				set_text(text_3, get(e).hint);
			});
			append($$anchor, span_1);
		};
		if_block(node_8, ($$render) => {
			if (get(e).hint) $$render(consequent_3);
		});
		Qualifier(sibling(node_8), { get q() {
			return get(e).qualifier;
		} });
		reset(button);
		reset(li_1);
		template_effect(() => {
			set_attribute(li_1, "aria-selected", i === get(at));
			set_text(text_2, get(e).label);
		});
		delegated("click", button, function(...$$args) {
			get(e).go?.apply(this, $$args);
		});
		event("mouseenter", button, () => set(at, i, true));
		append($$anchor, fragment);
	});
	var node_10 = sibling(node_5, 2);
	var consequent_4 = ($$anchor) => {
		var li_2 = root_4$10();
		var text_4 = only_child(li_2);
		template_effect(() => set_text(text_4, `Nothing by that name — ${get(entries).length ?? ""} things and ${get(bound) ?? ""} keys are here.`));
		append($$anchor, li_2);
	};
	if_block(node_10, ($$render) => {
		if (get(shown).length === 0) $$render(consequent_4);
	});
	reset(ul);
	next(2);
	reset(section);
	template_effect(() => set_attribute(input, "aria-activedescendant", get(shown)[get(at)] ? `palette-opt-${get(at)}` : void 0));
	delegated("keydown", input, typed);
	bind_value(input, () => get(query), ($$value) => set(query, $$value));
	append($$anchor, section);
	pop();
}
delegate(["keydown", "click"]);
//#endregion
//#region src/surfaces/palette/index.ts
var from = "";
onAction("open-palette", () => {
	if (location.hash.startsWith("#palette")) return true;
	from = location.hash;
	go("#palette");
	return true;
});
onAction("leave", (surface) => {
	if (surface !== "palette") return false;
	go(from || "");
	from = "";
	return true;
});
register({
	id: "palette",
	title: "Palette",
	heading: "Everything, by name",
	band: "happening",
	order: 9,
	nav: false,
	transient: true,
	reads: ["/api/changes", "/api/projects"],
	select: () => ({ from }),
	component: Palette
});
//#endregion
//#region src/surfaces/plan/store.svelte.ts
var specs = proxy({
	projects: null,
	omitted: 0,
	error: ""
});
var users = 0;
var timer = null;
async function read$1() {
	try {
		const r = await api("/api/specs");
		specs.projects = r.projects ?? [];
		specs.omitted = r.omitted ?? 0;
		specs.error = "";
	} catch (e) {
		specs.error = e instanceof Error ? e.message : String(e);
	}
}
function watch() {
	users += 1;
	if (users === 1) {
		read$1();
		timer = setInterval(read$1, 15e3);
	}
	return () => {
		users -= 1;
		if (users === 0 && timer) {
			clearInterval(timer);
			timer = null;
		}
	};
}
var key = (p, r) => `${p.project}/${r.plan.path}`;
//#endregion
//#region src/surfaces/plan/List.svelte
var root$15 = /* @__PURE__ */ from_html(`<p class="quiet fail svelte-16yypc0"> </p>`);
var root_1$15 = /* @__PURE__ */ from_html(`<div class="skel svelte-16yypc0"></div>`);
var root_2$12 = /* @__PURE__ */ from_html(`<span class="boxes svelte-16yypc0"> </span>`);
var root_3$10 = /* @__PURE__ */ from_html(`<!><!>`, 1);
var root_4$9 = /* @__PURE__ */ from_html(`<span class="wait svelte-16yypc0"><!> </span>`);
var root_5$7 = /* @__PURE__ */ from_html(`<span class="wait svelte-16yypc0"><!> drifted</span>`);
var root_6$7 = /* @__PURE__ */ from_html(`<button class="row svelte-16yypc0"><span class="title svelte-16yypc0"><!> </span> <span class="meta svelte-16yypc0"><!> <!> <!> <!></span></button>`);
var root_7$7 = /* @__PURE__ */ from_html(`<button class="group svelte-16yypc0"><!> <!> <span> </span> <span class="n svelte-16yypc0"> </span></button> <!>`, 1);
var root_8$7 = /* @__PURE__ */ from_html(`<details class="bare svelte-16yypc0"><summary class="svelte-16yypc0"><!> </summary> <p class="svelte-16yypc0"> </p> <p class="why svelte-16yypc0"><!></p></details>`);
var root_9$5 = /* @__PURE__ */ from_html(`<p class="none svelte-16yypc0"> </p>`);
var root_10$5 = /* @__PURE__ */ from_html(`<div class="list svelte-16yypc0"><header class="svelte-16yypc0"><span class="t svelte-16yypc0">Specifications</span><span class="n svelte-16yypc0"> </span></header> <label class="filter svelte-16yypc0"><!> <input placeholder="Filter specifications" aria-label="filter the specifications" class="svelte-16yypc0"/></label> <div class="rows svelte-16yypc0"><!> <!> <!> <!></div></div>`);
function List($$anchor, $$props) {
	push($$props, true);
	let focus = prop($$props, "focus", 3, "");
	user_effect(watch);
	let filter = /* @__PURE__ */ state("");
	let folded = /* @__PURE__ */ state(proxy({}));
	const name = (path) => path.split("/").filter(Boolean).pop() ?? path;
	const projects = /* @__PURE__ */ user_derived(() => (specs.projects ?? []).map((p) => ({
		...p,
		plans: p.plans.filter((r) => !get(filter).trim() || `${r.plan.path} ${r.title ?? ""}`.toLowerCase().includes(get(filter).trim().toLowerCase()))
	})).filter((p) => p.plans.length > 0));
	const bare = /* @__PURE__ */ user_derived(() => (specs.projects ?? []).filter((p) => p.plans.length === 0));
	const sentence = /* @__PURE__ */ user_derived(() => get(bare)[0]?.no_layout ?? "");
	const total = /* @__PURE__ */ user_derived(() => (specs.projects ?? []).reduce((n, p) => n + p.plans.length, 0));
	var div = root_10$5();
	var header = child(div);
	var text = only_child(sibling(child(header)), true);
	reset(header);
	var label = sibling(header, 2);
	var node = child(label);
	Icon(node, {
		name: "filter",
		size: 13
	});
	var input = sibling(node, 2);
	remove_input_defaults(input);
	reset(label);
	var div_1 = sibling(label, 2);
	var node_1 = child(div_1);
	var consequent = ($$anchor) => {
		var p_1 = root$15();
		var text_1 = only_child(p_1);
		template_effect(() => set_text(text_1, `The specifications could not be read: ${specs.error ?? ""}`));
		append($$anchor, p_1);
	};
	var consequent_1 = ($$anchor) => {
		var fragment = comment();
		each(first_child(fragment), 16, () => [
			0,
			1,
			2
		], (i) => i, ($$anchor, i) => {
			append($$anchor, root_1$15());
		});
		append($$anchor, fragment);
	};
	if_block(node_1, ($$render) => {
		if (specs.error) $$render(consequent);
		else if (specs.projects === null) $$render(consequent_1, 1);
	});
	var node_3 = sibling(node_1, 2);
	each(node_3, 17, () => get(projects), (p) => p.project_id, ($$anchor, p) => {
		var fragment_1 = root_7$7();
		var button = first_child(fragment_1);
		var node_4 = child(button);
		{
			let $0 = /* @__PURE__ */ user_derived(() => get(folded)[get(p).project_id] ? "right" : "down");
			Icon(node_4, {
				get name() {
					return get($0);
				},
				size: 12
			});
		}
		var node_5 = sibling(node_4, 2);
		Icon(node_5, {
			name: "folder",
			size: 13
		});
		var span_1 = sibling(node_5, 2);
		var text_2 = only_child(span_1, true);
		var text_3 = only_child(sibling(span_1, 2), true);
		reset(button);
		var node_6 = sibling(button, 2);
		var consequent_6 = ($$anchor) => {
			var fragment_2 = comment();
			each(first_child(fragment_2), 17, () => get(p).plans, (r) => r.plan.path, ($$anchor, r) => {
				const k = /* @__PURE__ */ user_derived(() => key(get(p), get(r)));
				var button_1 = root_6$7();
				var span_3 = child(button_1);
				var node_8 = child(span_3);
				Icon(node_8, {
					name: "spec",
					size: 13
				});
				var text_4 = sibling(node_8);
				reset(span_3);
				var span_4 = sibling(span_3, 2);
				var node_9 = child(span_4);
				var consequent_2 = ($$anchor) => {
					var span_5 = root_2$12();
					var text_5 = only_child(span_5);
					template_effect(() => {
						set_attribute(span_5, "title", `boxes ticked in the task file · ${get(r).plan.progress.total ?? ""} tasks`);
						set_text(text_5, `${get(r).plan.progress.done ?? ""} ticked`);
					});
					append($$anchor, span_5);
				};
				if_block(node_9, ($$render) => {
					if (get(r).plan.progress) $$render(consequent_2);
				});
				var node_10 = sibling(node_9, 2);
				var consequent_3 = ($$anchor) => {
					var fragment_3 = root_3$10();
					var node_11 = first_child(fragment_3);
					Pill(node_11, { get word() {
						return get(r).state;
					} });
					Qualifier(sibling(node_11), { get q() {
						return get(r).qualifier;
					} });
					append($$anchor, fragment_3);
				};
				if_block(node_10, ($$render) => {
					if (get(r).state) $$render(consequent_3);
				});
				var node_13 = sibling(node_10, 2);
				var consequent_4 = ($$anchor) => {
					var span_6 = root_4$9();
					var node_14 = child(span_6);
					Icon(node_14, {
						name: "question",
						size: 11
					});
					var text_6 = sibling(node_14);
					reset(span_6);
					template_effect(() => set_text(text_6, ` ${get(r).plan.open_questions ?? ""}`));
					append($$anchor, span_6);
				};
				if_block(node_13, ($$render) => {
					if (get(r).plan.open_questions > 0) $$render(consequent_4);
				});
				var node_15 = sibling(node_13, 2);
				var consequent_5 = ($$anchor) => {
					var span_7 = root_5$7();
					Icon(child(span_7), {
						name: "alert",
						size: 11
					});
					next();
					reset(span_7);
					append($$anchor, span_7);
				};
				if_block(node_15, ($$render) => {
					if (get(r).drifted) $$render(consequent_5);
				});
				reset(span_4);
				reset(button_1);
				template_effect(($0) => {
					set_attribute(button_1, "aria-current", get(k) === focus() ? "true" : void 0);
					set_text(text_4, ` ${$0 ?? ""}`);
				}, [() => name(get(r).plan.path)]);
				delegated("click", button_1, () => $$props.open(get(k)));
				delegated("dblclick", button_1, () => $$props.open(get(k), true));
				append($$anchor, button_1);
			});
			append($$anchor, fragment_2);
		};
		if_block(node_6, ($$render) => {
			if (!get(folded)[get(p).project_id]) $$render(consequent_6);
		});
		template_effect(() => {
			set_attribute(button, "aria-expanded", !get(folded)[get(p).project_id]);
			set_text(text_2, get(p).project);
			set_text(text_3, get(p).plans.length);
		});
		delegated("click", button, () => set(folded, {
			...get(folded),
			[get(p).project_id]: !get(folded)[get(p).project_id]
		}, true));
		append($$anchor, fragment_1);
	});
	var node_17 = sibling(node_3, 2);
	var consequent_7 = ($$anchor) => {
		var details = root_8$7();
		var summary = child(details);
		var node_18 = child(summary);
		Icon(node_18, {
			name: "folder",
			size: 13
		});
		var text_7 = sibling(node_18);
		reset(summary);
		var p_2 = sibling(summary, 2);
		var text_8 = only_child(p_2, true);
		var p_3 = sibling(p_2, 2);
		Inline(child(p_3), { get text() {
			return get(sentence);
		} });
		reset(p_3);
		reset(details);
		template_effect(($0) => {
			set_text(text_7, ` ${get(bare).length ?? ""} ${get(bare).length === 1 ? "project has" : "projects have"} no specification layout`);
			set_text(text_8, $0);
		}, [() => get(bare).map((p) => p.project).join(", ")]);
		append($$anchor, details);
	};
	var d = /* @__PURE__ */ user_derived(() => get(bare).length > 0 && !get(filter).trim());
	if_block(node_17, ($$render) => {
		if (get(d)) $$render(consequent_7);
	});
	var node_20 = sibling(node_17, 2);
	var consequent_8 = ($$anchor) => {
		var p_4 = root_9$5();
		var text_9 = only_child(p_4);
		template_effect(() => set_text(text_9, `${specs.omitted ?? ""} more not shown.`));
		append($$anchor, p_4);
	};
	if_block(node_20, ($$render) => {
		if (specs.omitted > 0) $$render(consequent_8);
	});
	reset(div_1);
	reset(div);
	template_effect(() => set_text(text, get(total)));
	bind_value(input, () => get(filter), ($$value) => set(filter, $$value));
	append($$anchor, div);
	pop();
}
delegate(["click", "dblclick"]);
//#endregion
//#region src/surfaces/plan/Doc.svelte
var root$14 = /* @__PURE__ */ from_html(`<div class="pad quiet svelte-khab3i">Reading the specifications…</div>`);
var root_1$14 = /* @__PURE__ */ from_html(`<span class="svelte-khab3i"><b> </b> boxes · <b> </b> ticked</span>`);
var root_2$11 = /* @__PURE__ */ from_html(`<span class="wait svelte-khab3i"><!> </span>`);
var root_3$9 = /* @__PURE__ */ from_html(`<span class="wait svelte-khab3i"><!> changed under a run</span>`);
var root_4$8 = /* @__PURE__ */ from_html(`<!><!>`, 1);
var root_5$6 = /* @__PURE__ */ from_html(`<span class="quiet svelte-khab3i"> </span>`);
var root_6$6 = /* @__PURE__ */ from_html(`<a class="btn svelte-khab3i"><!> </a> <!> <!>`, 1);
var root_7$6 = /* @__PURE__ */ from_html(`<button class="btn primary svelte-khab3i"><!> Start a change…</button> <span class="quiet svelte-khab3i">No change is working to this specification; pick it in the form.</span>`, 1);
var root_8$6 = /* @__PURE__ */ from_html(`<p class="warn svelte-khab3i"><!> Its change is finished while boxes are still open.</p>`);
var root_9$4 = /* @__PURE__ */ from_html(`<li> </li>`);
var root_10$4 = /* @__PURE__ */ from_html(`<section class="card wait svelte-khab3i"><h2 class="svelte-khab3i"><!> Waiting on a person</h2> <ul class="svelte-khab3i"></ul></section>`);
var root_11$3 = /* @__PURE__ */ from_html(`<p class="quiet svelte-khab3i">Not traced.</p>`);
var root_12$1 = /* @__PURE__ */ from_html(`<p class="quiet svelte-khab3i">No requirement notation was recognised in this folder. A project declares its own with <code class="svelte-khab3i">[spec] tokens</code> in devplane.toml; nothing is guessed.</p>`);
var root_13$1 = /* @__PURE__ */ from_html(`<tr><td class="svelte-khab3i"><code class="svelte-khab3i"> </code></td><td class="svelte-khab3i"> </td><td class="svelte-khab3i"> </td></tr>`);
var root_14$1 = /* @__PURE__ */ from_html(`<p class="warn svelte-khab3i"> </p>`);
var root_15$1 = /* @__PURE__ */ from_html(`<p class="quiet svelte-khab3i"> </p>`);
var root_16 = /* @__PURE__ */ from_html(`<table class="svelte-khab3i"><thead><tr><th class="svelte-khab3i">requirement</th><th class="svelte-khab3i">tasks</th><th class="svelte-khab3i">ticked</th></tr></thead><tbody></tbody></table> <!> <!>`, 1);
var root_17 = /* @__PURE__ */ from_html(`<article class="doc svelte-khab3i"><header class="svelte-khab3i"><p class="where svelte-khab3i"><!> <span>/</span> <code class="svelte-khab3i"> </code></p> <h1 class="svelte-khab3i"> </h1> <div class="facts svelte-khab3i"><!> <span class="svelte-khab3i"> </span> <!> <!></div> <div class="acts svelte-khab3i"><!></div> <!></header> <div class="cols svelte-khab3i"><nav class="outline svelte-khab3i" aria-label="outline"><h2 class="svelte-khab3i">Outline</h2> <ol class="svelte-khab3i"></ol></nav> <div class="main svelte-khab3i"><!> <section class="card svelte-khab3i"><h2 class="svelte-khab3i">Requirements and the tasks that cite them</h2> <!></section></div></div></article>`);
function Doc($$anchor, $$props) {
	push($$props, true);
	let focus = prop($$props, "focus", 3, "");
	user_effect(watch);
	const found = /* @__PURE__ */ user_derived(() => {
		for (const p of specs.projects ?? []) for (const r of p.plans) if (key(p, r) === focus()) return {
			p,
			r
		};
		return null;
	});
	const total = /* @__PURE__ */ user_derived(() => (specs.projects ?? []).reduce((n, p) => n + p.plans.length, 0));
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_1 = ($$anchor) => {
		var fragment_1 = comment();
		var node_1 = first_child(fragment_1);
		var consequent = ($$anchor) => {
			append($$anchor, root$14());
		};
		var alternate = ($$anchor) => {
			{
				let $0 = /* @__PURE__ */ user_derived(() => get(total) ? "Pick a specification" : "No specification on this machine");
				let $1 = /* @__PURE__ */ user_derived(() => get(total) ? `${get(total)} specifications across ${specs.projects.length} projects. A specification is read as it is on disk: its outline, its boxes, and the requirements its tasks cite.` : "Devplane reads Spec Kit, OpenSpec and Kiro folders where they already are, and a plain folder a project names in devplane.toml.");
				Empty($$anchor, {
					icon: "spec",
					get title() {
						return get($0);
					},
					get body() {
						return get($1);
					}
				});
			}
		};
		if_block(node_1, ($$render) => {
			if (specs.projects === null) $$render(consequent);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment_1);
	};
	var alternate_3 = ($$anchor) => {
		const r = /* @__PURE__ */ user_derived(() => get(found).r);
		var article = root_17();
		var header = child(article);
		var p_1 = child(header);
		var node_2 = child(p_1);
		Icon(node_2, {
			name: "folder",
			size: 12
		});
		var text = sibling(node_2);
		var text_1 = only_child(sibling(text, 3), true);
		reset(p_1);
		var h1 = sibling(p_1, 2);
		var text_2 = only_child(h1, true);
		var div_1 = sibling(h1, 2);
		var node_3 = child(div_1);
		var consequent_2 = ($$anchor) => {
			var span = root_1$14();
			var b = child(span);
			var text_3 = only_child(b, true);
			var text_4 = only_child(sibling(b, 2), true);
			next();
			reset(span);
			template_effect(() => {
				set_text(text_3, get(r).plan.progress.total);
				set_text(text_4, get(r).plan.progress.done);
			});
			append($$anchor, span);
		};
		if_block(node_3, ($$render) => {
			if (get(r).plan.progress) $$render(consequent_2);
		});
		var span_1 = sibling(node_3, 2);
		var text_5 = only_child(span_1);
		var node_4 = sibling(span_1, 2);
		var consequent_3 = ($$anchor) => {
			var span_2 = root_2$11();
			var node_5 = child(span_2);
			Icon(node_5, {
				name: "question",
				size: 13
			});
			var text_6 = sibling(node_5);
			reset(span_2);
			template_effect(() => set_text(text_6, ` ${get(r).plan.open_questions ?? ""} open questions`));
			append($$anchor, span_2);
		};
		if_block(node_4, ($$render) => {
			if (get(r).plan.open_questions > 0) $$render(consequent_3);
		});
		var node_6 = sibling(node_4, 2);
		var consequent_4 = ($$anchor) => {
			var span_3 = root_3$9();
			Icon(child(span_3), {
				name: "alert",
				size: 13
			});
			next();
			reset(span_3);
			append($$anchor, span_3);
		};
		if_block(node_6, ($$render) => {
			if (get(r).drifted) $$render(consequent_4);
		});
		reset(div_1);
		var div_2 = sibling(div_1, 2);
		var node_8 = child(div_2);
		var consequent_7 = ($$anchor) => {
			var fragment_3 = root_6$6();
			var a = first_child(fragment_3);
			var node_9 = child(a);
			Icon(node_9, {
				name: "change",
				size: 14
			});
			var text_7 = sibling(node_9);
			reset(a);
			var node_10 = sibling(a, 2);
			var consequent_5 = ($$anchor) => {
				var fragment_4 = root_4$8();
				var node_11 = first_child(fragment_4);
				Pill(node_11, { get word() {
					return get(r).state;
				} });
				Qualifier(sibling(node_11), { get q() {
					return get(r).qualifier;
				} });
				append($$anchor, fragment_4);
			};
			if_block(node_10, ($$render) => {
				if (get(r).state) $$render(consequent_5);
			});
			var node_13 = sibling(node_10, 2);
			var consequent_6 = ($$anchor) => {
				var span_4 = root_5$6();
				var text_8 = only_child(span_4, true);
				template_effect(() => set_text(text_8, get(r).counts_says));
				append($$anchor, span_4);
			};
			if_block(node_13, ($$render) => {
				if (get(r).counts_says) $$render(consequent_6);
			});
			template_effect(($0) => {
				set_attribute(a, "href", $0);
				set_text(text_7, ` ${get(r).title ?? ""}`);
			}, [() => `#change/${encodeURIComponent(get(r).change_id)}`]);
			append($$anchor, fragment_3);
		};
		var alternate_1 = ($$anchor) => {
			var fragment_5 = root_7$6();
			var button = first_child(fragment_5);
			Icon(child(button), {
				name: "play",
				size: 14
			});
			next();
			reset(button);
			next(2);
			delegated("click", button, () => run("new-change", "plan"));
			append($$anchor, fragment_5);
		};
		if_block(node_8, ($$render) => {
			if (get(r).change_id) $$render(consequent_7);
			else $$render(alternate_1, -1);
		});
		reset(div_2);
		var node_15 = sibling(div_2, 2);
		var consequent_8 = ($$anchor) => {
			var p_2 = root_8$6();
			Icon(child(p_2), {
				name: "alert",
				size: 13
			});
			next();
			reset(p_2);
			append($$anchor, p_2);
		};
		if_block(node_15, ($$render) => {
			if (get(r).contradicts_done) $$render(consequent_8);
		});
		reset(header);
		var div_3 = sibling(header, 2);
		var nav = child(div_3);
		var ol = sibling(child(nav), 2);
		each(ol, 21, () => get(r).plan.outline, index, ($$anchor, h) => {
			var li = root_9$4();
			let styles;
			var text_9 = only_child(li, true);
			template_effect(($0, $1) => {
				set_class(li, 1, `h${$0 ?? ""}`, "svelte-khab3i");
				styles = set_style(li, "", styles, { "padding-left": $1 });
				set_text(text_9, get(h).text);
			}, [() => Math.min(get(h).level, 4), () => `${Math.max(0, get(h).level - 1) * .8}rem`]);
			append($$anchor, li);
		});
		reset(ol);
		reset(nav);
		var div_4 = sibling(nav, 2);
		var node_17 = child(div_4);
		var consequent_9 = ($$anchor) => {
			var section = root_10$4();
			var h2 = child(section);
			Icon(child(h2), {
				name: "question",
				size: 13
			});
			next();
			reset(h2);
			var ul = sibling(h2, 2);
			each(ul, 21, () => get(r).plan.questions, index, ($$anchor, q) => {
				var li_1 = root_9$4();
				var text_10 = only_child(li_1, true);
				template_effect(($0) => set_text(text_10, $0), [() => typeof get(q) === "string" ? get(q) : JSON.stringify(get(q))]);
				append($$anchor, li_1);
			});
			reset(ul);
			reset(section);
			append($$anchor, section);
		};
		if_block(node_17, ($$render) => {
			if (get(r).plan.questions?.length) $$render(consequent_9);
		});
		var section_1 = sibling(node_17, 2);
		var node_19 = sibling(child(section_1), 2);
		var consequent_10 = ($$anchor) => {
			append($$anchor, root_11$3());
		};
		var consequent_11 = ($$anchor) => {
			append($$anchor, root_12$1());
		};
		var alternate_2 = ($$anchor) => {
			var fragment_6 = root_16();
			var table = first_child(fragment_6);
			var tbody = sibling(child(table));
			each(tbody, 21, () => get(r).trace.edges, ([token, tasks]) => token, ($$anchor, $$item) => {
				var $$array = /* @__PURE__ */ user_derived(() => to_array(get($$item), 2));
				let token = () => get($$array)[0];
				let tasks = () => get($$array)[1];
				var tr = root_13$1();
				var td = child(tr);
				var text_11 = only_child(child(td), true);
				reset(td);
				var td_1 = sibling(td);
				var text_12 = only_child(td_1, true);
				var text_13 = only_child(sibling(td_1), true);
				reset(tr);
				template_effect(($0) => {
					set_text(text_11, token());
					set_text(text_12, tasks().length);
					set_text(text_13, $0);
				}, [() => tasks().filter((t) => t.done).length]);
				append($$anchor, tr);
			});
			reset(tbody);
			reset(table);
			var node_20 = sibling(table, 2);
			var consequent_12 = ($$anchor) => {
				var p_5 = root_14$1();
				var text_14 = only_child(p_5);
				template_effect(($0) => set_text(text_14, `No task cites: ${$0 ?? ""}`), [() => get(r).trace.orphan_requirements.join(", ")]);
				append($$anchor, p_5);
			};
			if_block(node_20, ($$render) => {
				if (get(r).trace.orphan_requirements.length) $$render(consequent_12);
			});
			var node_21 = sibling(node_20, 2);
			var consequent_13 = ($$anchor) => {
				var p_6 = root_15$1();
				var text_15 = only_child(p_6);
				template_effect(() => set_text(text_15, `${get(r).trace.tasks_citing_nothing.length ?? ""} tasks cite no requirement.`));
				append($$anchor, p_6);
			};
			if_block(node_21, ($$render) => {
				if (get(r).trace.tasks_citing_nothing.length) $$render(consequent_13);
			});
			append($$anchor, fragment_6);
		};
		if_block(node_19, ($$render) => {
			if (!get(r).trace) $$render(consequent_10);
			else if (get(r).trace.unrecognised) $$render(consequent_11, 1);
			else $$render(alternate_2, -1);
		});
		reset(section_1);
		reset(div_4);
		reset(div_3);
		reset(article);
		template_effect(() => {
			set_text(text, ` ${get(found).p.project ?? ""} `);
			set_text(text_1, get(r).plan.path);
			set_text(text_2, get(r).plan.outline[0]?.text ?? get(r).plan.path);
			set_text(text_5, `${get(r).plan.files ?? ""} files`);
		});
		append($$anchor, article);
	};
	if_block(node, ($$render) => {
		if (!focus() || !get(found)) $$render(consequent_1);
		else $$render(alternate_3, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/plan/index.ts
bind({
	surface: "global",
	combo: "g s",
	action: "go-specs",
	label: "go to the specifications"
});
onAction("go-specs", () => {
	go("#plan");
	return true;
});
register({
	id: "plan",
	icon: "spec",
	title: "Specifications",
	heading: "Specifications",
	band: "happening",
	order: 3,
	reads: ["/api/specs"],
	tab: (_feed, focus) => focus.split("/").filter(Boolean).pop() ?? "Specification",
	select: () => ({}),
	side: List,
	component: Doc
});
//#endregion
//#region src/surfaces/quit/Quit.svelte
var root$13 = /* @__PURE__ */ from_html(`<pre class="says svelte-1pz5pdz"> </pre>`);
var root_1$13 = /* @__PURE__ */ from_html(`<p class="says warn svelte-1pz5pdz"> </p>`);
var root_2$10 = /* @__PURE__ */ from_html(`<p class="dim svelte-1pz5pdz">reading what this would stop…</p>`);
var root_3$8 = /* @__PURE__ */ from_html(`<section class="quit svelte-1pz5pdz" aria-labelledby="quit-head"><h2 id="quit-head" class="svelte-1pz5pdz">Quit Devplane?</h2> <!> <div class="acts svelte-1pz5pdz"><button class="primary"> </button> <button>Keep running</button></div></section>`);
function Quit($$anchor, $$props) {
	push($$props, true);
	let says = prop($$props, "says", 3, null);
	const read = resource(() => "/api/quitting", { tell: () => "devplane quit" });
	let refused = /* @__PURE__ */ state("");
	let stopping = /* @__PURE__ */ state(false);
	const shown = /* @__PURE__ */ user_derived(() => read.data ? read.data.says ?? "" : says());
	const unreadable = /* @__PURE__ */ user_derived(() => get(refused) || (read.failure ? `Could not read what this would stop (${read.failure.says}). Quitting anyway ends anything it started.` : ""));
	user_effect(() => onAction("leave", () => {
		keep();
		return true;
	}));
	async function quit() {
		set(stopping, true);
		try {
			await api("/api/quit", { method: "POST" });
		} catch (e) {
			set(stopping, false);
			set(refused, `that did not land: ${failure(e).says}`);
		}
	}
	function keep() {
		hideWindow();
	}
	var section = root_3$8();
	var node = sibling(child(section), 2);
	var consequent = ($$anchor) => {
		var pre = root$13();
		var text = only_child(pre, true);
		template_effect(() => set_text(text, get(shown)));
		append($$anchor, pre);
	};
	var consequent_1 = ($$anchor) => {
		var p = root_1$13();
		var text_1 = only_child(p, true);
		template_effect(() => set_text(text_1, get(unreadable)));
		append($$anchor, p);
	};
	var alternate = ($$anchor) => {
		append($$anchor, root_2$10());
	};
	if_block(node, ($$render) => {
		if (get(shown) !== null) $$render(consequent);
		else if (get(unreadable)) $$render(consequent_1, 1);
		else $$render(alternate, -1);
	});
	var div = sibling(node, 2);
	var button = child(div);
	var text_2 = only_child(button, true);
	var button_1 = sibling(button, 2);
	reset(div);
	reset(section);
	template_effect(() => {
		button.disabled = get(stopping);
		set_text(text_2, get(stopping) ? "stopping…" : "Quit");
		button_1.disabled = get(stopping);
	});
	delegated("click", button, quit);
	delegated("click", button_1, keep);
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/quit/index.ts
register({
	id: "quit",
	title: "Quit",
	heading: "Quit Devplane?",
	band: "project",
	order: 9,
	nav: false,
	bare: true,
	reads: ["/api/quitting"],
	select: () => ({}),
	component: Quit
});
//#endregion
//#region src/surfaces/reports/ReportList.svelte
var root$12 = /* @__PURE__ */ from_html(`<option> </option>`);
var root_1$12 = /* @__PURE__ */ from_html(`<p class="said svelte-s9iigm" role="status"> </p>`);
var root_2$9 = /* @__PURE__ */ from_html(`<p class="warn svelte-s9iigm"> </p>`);
var root_3$7 = /* @__PURE__ */ from_html(`<form class="file svelte-s9iigm" aria-label="file a report"><div class="two svelte-s9iigm"><label class="svelte-s9iigm">From <select><option>choose</option><!></select></label> <label class="svelte-s9iigm">To <input placeholder="a registered project, or owner/repo on GitHub"/></label> <label class="svelte-s9iigm">Kind <select><option>defect</option><option>question</option><option>request</option><option>breaking change</option></select></label></div> <label class="svelte-s9iigm">Title <input placeholder="One line a stranger would understand"/></label> <label class="svelte-s9iigm">Finding <textarea rows="4" placeholder="What is wrong, and how you know"></textarea></label> <div class="two svelte-s9iigm"><label class="svelte-s9iigm">Command that shows it <input placeholder="optional"/></label> <label class="svelte-s9iigm">Its output <input placeholder="optional"/></label></div> <!> <div class="row svelte-s9iigm"><button type="submit" class="primary svelte-s9iigm">File it</button><button type="button" class="svelte-s9iigm">Cancel</button></div></form>`);
var root_4$7 = /* @__PURE__ */ from_html(`<p class="quiet svelte-s9iigm">Reading…</p>`);
var root_5$5 = /* @__PURE__ */ from_html(`<b class="t svelte-s9iigm"> </b>`);
var root_6$5 = /* @__PURE__ */ from_html(`<span class="dim svelte-s9iigm"> </span>`);
var root_7$5 = /* @__PURE__ */ from_html(` <span class="dim svelte-s9iigm">→</span> `, 1);
var root_8$5 = /* @__PURE__ */ from_html(`<button class="svelte-s9iigm"><!> Open the drafted issue</button>`);
var root_9$3 = /* @__PURE__ */ from_html(`<aside class="drawer svelte-s9iigm"><header class="svelte-s9iigm"><b> </b><button class="x svelte-s9iigm" aria-label="close"><!></button></header> <p class="dim svelte-s9iigm"> </p> <p><!></p> <pre class="quoted svelte-s9iigm" aria-label="the report, quoted as it was filed"> </pre> <div class="acts svelte-s9iigm"><button class="svelte-s9iigm"><!> Start a change from it</button> <button class="svelte-s9iigm"><!> Fixed</button> <button class="svelte-s9iigm"><!> Rejected</button> <button class="svelte-s9iigm"><!> Deferred</button> <!></div></aside>`);
var root_10$3 = /* @__PURE__ */ from_html(`<div class="frame svelte-s9iigm"><!> <!></div>`);
var root_11$2 = /* @__PURE__ */ from_html(`<div class="page svelte-s9iigm"><header class="head svelte-s9iigm"><h1 class="svelte-s9iigm">Reports</h1> <select aria-label="which project"><option>every project</option><!></select> <span class="gap svelte-s9iigm"></span> <button class="primary svelte-s9iigm"><!> File a report</button></header> <!> <!> <!></div>`);
function ReportList($$anchor, $$props) {
	push($$props, true);
	let reports = prop($$props, "reports", 19, () => []), projects = prop($$props, "projects", 19, () => []), loaded = prop($$props, "loaded", 3, false), failed = prop($$props, "failed", 3, ""), said = prop($$props, "said", 3, ""), file = prop($$props, "file", 3, async () => false), reload = prop($$props, "reload", 3, async () => {});
	let about = /* @__PURE__ */ state("");
	let selected = /* @__PURE__ */ state(null);
	let filing = /* @__PURE__ */ state(false);
	let acted = /* @__PURE__ */ state("");
	const shown = /* @__PURE__ */ user_derived(() => get(about) ? reports().filter((r) => r.target.project === get(about) || r.provenance.project === get(about)) : reports());
	const pick = /* @__PURE__ */ user_derived(() => reports().find((r) => r.id === get(selected)) ?? null);
	const columns = [
		{
			key: "title",
			label: "Finding",
			width: 320,
			sort: (r) => r.title
		},
		{
			key: "kind",
			label: "Kind",
			width: 90,
			sort: (r) => r.kind
		},
		{
			key: "route",
			label: "From → to",
			width: 220,
			sort: (r) => r.provenance.project_name
		},
		{
			key: "state",
			label: "State",
			width: 200,
			sort: (r) => r.state_says
		},
		{
			key: "age",
			label: "Filed",
			align: "end",
			sort: (r) => r.age_says
		}
	];
	async function act(r, verb, how) {
		try {
			const id = encodeURIComponent(r.id);
			const res = await api(verb === "start" ? `/api/reports/${id}/start` : verb === "open" ? `/api/reports/${id}/open` : `/api/reports/${id}/resolve`, {
				method: "POST",
				body: JSON.stringify(verb === "resolve" ? { as: how } : {})
			});
			set(acted, res.says ?? (verb === "start" ? "A change was started from it." : verb === "open" ? "Opened on GitHub." : `Marked ${how}.`), true);
			await reload()();
		} catch (e) {
			set(acted, `That did not land: ${e instanceof Error ? e.message : String(e)}`);
		}
	}
	let f = /* @__PURE__ */ state(proxy({
		to: "",
		from: "",
		kind: "defect",
		title: "",
		words: "",
		command: "",
		output: ""
	}));
	let refused = /* @__PURE__ */ state("");
	async function submit(e) {
		e.preventDefault();
		if (!get(f).to.trim() || !get(f).from || !get(f).title.trim() || !get(f).words.trim()) {
			set(refused, "Say where it goes, where it comes from, a title and the finding.");
			return;
		}
		set(refused, "");
		if (await file()({ ...get(f) })) {
			set(f, {
				...get(f),
				title: "",
				words: "",
				command: "",
				output: ""
			}, true);
			set(filing, false);
		}
	}
	var div = root_11$2();
	var header = child(div);
	var select = sibling(child(header), 2);
	var option = child(select);
	option.value = option.__value = "";
	each(sibling(option), 17, projects, (p) => p.id, ($$anchor, p) => {
		var option_1 = root$12();
		var text = only_child(option_1, true);
		var option_1_value = {};
		template_effect(() => {
			set_text(text, get(p).name);
			if (option_1_value !== (option_1_value = get(p).id)) option_1.value = (option_1.__value = option_1_value) ?? "";
		});
		append($$anchor, option_1);
	});
	reset(select);
	init_select(select);
	var button = sibling(select, 4);
	Icon(child(button), {
		name: "plus",
		size: 14
	});
	next();
	reset(button);
	reset(header);
	var node_2 = sibling(header, 2);
	var consequent = ($$anchor) => {
		var p_1 = root_1$12();
		var text_1 = only_child(p_1, true);
		template_effect(() => set_text(text_1, get(acted) || said()));
		append($$anchor, p_1);
	};
	if_block(node_2, ($$render) => {
		if (said() || get(acted)) $$render(consequent);
	});
	var node_3 = sibling(node_2, 2);
	var consequent_2 = ($$anchor) => {
		var form = root_3$7();
		var div_1 = child(form);
		var label = child(div_1);
		var select_1 = sibling(child(label));
		var option_2 = child(select_1);
		option_2.value = option_2.__value = "";
		each(sibling(option_2), 17, projects, (p) => p.id, ($$anchor, p) => {
			var option_3 = root$12();
			var text_2 = only_child(option_3, true);
			var option_3_value = {};
			template_effect(() => {
				set_text(text_2, get(p).name);
				if (option_3_value !== (option_3_value = get(p).id)) option_3.value = (option_3.__value = option_3_value) ?? "";
			});
			append($$anchor, option_3);
		});
		reset(select_1);
		init_select(select_1);
		reset(label);
		var label_1 = sibling(label, 2);
		var input = sibling(child(label_1));
		remove_input_defaults(input);
		reset(label_1);
		var label_2 = sibling(label_1, 2);
		var select_2 = sibling(child(label_2));
		var option_4 = child(select_2);
		option_4.value = option_4.__value = "defect";
		var option_5 = sibling(option_4);
		option_5.value = option_5.__value = "question";
		var option_6 = sibling(option_5);
		option_6.value = option_6.__value = "request";
		var option_7 = sibling(option_6);
		option_7.value = option_7.__value = "breaking_change";
		reset(select_2);
		init_select(select_2);
		reset(label_2);
		reset(div_1);
		var label_3 = sibling(div_1, 2);
		var input_1 = sibling(child(label_3));
		remove_input_defaults(input_1);
		reset(label_3);
		var label_4 = sibling(label_3, 2);
		var textarea = sibling(child(label_4));
		remove_textarea_child(textarea);
		reset(label_4);
		var div_2 = sibling(label_4, 2);
		var label_5 = child(div_2);
		var input_2 = sibling(child(label_5));
		remove_input_defaults(input_2);
		reset(label_5);
		var label_6 = sibling(label_5, 2);
		var input_3 = sibling(child(label_6));
		remove_input_defaults(input_3);
		reset(label_6);
		reset(div_2);
		var node_5 = sibling(div_2, 2);
		var consequent_1 = ($$anchor) => {
			var p_2 = root_2$9();
			var text_3 = only_child(p_2, true);
			template_effect(() => set_text(text_3, get(refused)));
			append($$anchor, p_2);
		};
		if_block(node_5, ($$render) => {
			if (get(refused)) $$render(consequent_1);
		});
		var div_3 = sibling(node_5, 2);
		var button_1 = sibling(child(div_3));
		reset(div_3);
		reset(form);
		event("submit", form, submit);
		bind_select_value(select_1, () => get(f).from, ($$value) => get(f).from = $$value);
		bind_value(input, () => get(f).to, ($$value) => get(f).to = $$value);
		bind_select_value(select_2, () => get(f).kind, ($$value) => get(f).kind = $$value);
		bind_value(input_1, () => get(f).title, ($$value) => get(f).title = $$value);
		bind_value(textarea, () => get(f).words, ($$value) => get(f).words = $$value);
		bind_value(input_2, () => get(f).command, ($$value) => get(f).command = $$value);
		bind_value(input_3, () => get(f).output, ($$value) => get(f).output = $$value);
		delegated("click", button_1, () => set(filing, false));
		append($$anchor, form);
	};
	if_block(node_3, ($$render) => {
		if (get(filing)) $$render(consequent_2);
	});
	var node_6 = sibling(node_3, 2);
	var consequent_3 = ($$anchor) => {
		append($$anchor, root_4$7());
	};
	var consequent_4 = ($$anchor) => {
		Empty($$anchor, {
			icon: "alert",
			title: "The reports could not be read",
			get body() {
				return failed();
			}
		});
	};
	var consequent_5 = ($$anchor) => {
		Empty($$anchor, {
			icon: "report",
			title: "No report has been filed",
			body: "An agent working on one project can file what it found about another — it reaches that project's person, quoted and with where it came from."
		});
	};
	var alternate_1 = ($$anchor) => {
		var div_4 = root_10$3();
		var node_7 = child(div_4);
		{
			const cell = ($$anchor, r = noop, c = noop) => {
				var fragment_2 = comment();
				var node_8 = first_child(fragment_2);
				var consequent_6 = ($$anchor) => {
					var b = root_5$5();
					var text_4 = only_child(b, true);
					template_effect(() => set_text(text_4, r().title));
					append($$anchor, b);
				};
				var consequent_7 = ($$anchor) => {
					var span = root_6$5();
					var text_5 = only_child(span, true);
					template_effect(() => set_text(text_5, r().kind));
					append($$anchor, span);
				};
				var consequent_8 = ($$anchor) => {
					var fragment_3 = root_7$5();
					var text_6 = first_child(fragment_3);
					var text_7 = sibling(text_6, 2);
					template_effect(() => {
						set_text(text_6, `${r().provenance.project_name ?? ""} `);
						set_text(text_7, ` ${r().target_says ?? ""}`);
					});
					append($$anchor, fragment_3);
				};
				var consequent_9 = ($$anchor) => {
					Pill($$anchor, { get word() {
						return r().state_says;
					} });
				};
				var alternate = ($$anchor) => {
					var span_1 = root_6$5();
					var text_8 = only_child(span_1, true);
					template_effect(() => set_text(text_8, r().age_says));
					append($$anchor, span_1);
				};
				if_block(node_8, ($$render) => {
					if (c().key === "title") $$render(consequent_6);
					else if (c().key === "kind") $$render(consequent_7, 1);
					else if (c().key === "route") $$render(consequent_8, 2);
					else if (c().key === "state") $$render(consequent_9, 3);
					else $$render(alternate, -1);
				});
				append($$anchor, fragment_2);
			};
			Grid(node_7, {
				id: "reports",
				get columns() {
					return columns;
				},
				get rows() {
					return get(shown);
				},
				key: (r) => r.id,
				label: "reports",
				get selected() {
					return get(selected);
				},
				set selected($$value) {
					set(selected, $$value, true);
				},
				cell,
				$$slots: { cell: true }
			});
		}
		var node_9 = sibling(node_7, 2);
		var consequent_11 = ($$anchor) => {
			var aside = root_9$3();
			var header_1 = child(aside);
			var b_1 = child(header_1);
			var text_9 = only_child(b_1, true);
			var button_2 = sibling(b_1);
			Icon(child(button_2), {
				name: "x",
				size: 13
			});
			reset(button_2);
			reset(header_1);
			var p_4 = sibling(header_1, 2);
			var text_10 = only_child(p_4, true);
			var p_5 = sibling(p_4, 2);
			Pill(child(p_5), { get word() {
				return get(pick).state_says;
			} });
			reset(p_5);
			var pre = sibling(p_5, 2);
			var text_11 = only_child(pre, true);
			var div_5 = sibling(pre, 2);
			var button_3 = child(div_5);
			Icon(child(button_3), {
				name: "play",
				size: 13
			});
			next();
			reset(button_3);
			var button_4 = sibling(button_3, 2);
			Icon(child(button_4), {
				name: "check",
				size: 13
			});
			next();
			reset(button_4);
			var button_5 = sibling(button_4, 2);
			Icon(child(button_5), {
				name: "x",
				size: 13
			});
			next();
			reset(button_5);
			var button_6 = sibling(button_5, 2);
			Icon(child(button_6), {
				name: "clock",
				size: 13
			});
			next();
			reset(button_6);
			var node_16 = sibling(button_6, 2);
			var consequent_10 = ($$anchor) => {
				var button_7 = root_8$5();
				Icon(child(button_7), {
					name: "forge",
					size: 13
				});
				next();
				reset(button_7);
				delegated("click", button_7, () => act(get(pick), "open"));
				append($$anchor, button_7);
			};
			var d = /* @__PURE__ */ user_derived(() => get(pick).target.to.includes("/"));
			if_block(node_16, ($$render) => {
				if (get(d)) $$render(consequent_10);
			});
			reset(div_5);
			reset(aside);
			template_effect(() => {
				set_text(text_9, get(pick).title);
				set_text(text_10, get(pick).provenance_says);
				set_text(text_11, get(pick).quoted);
			});
			delegated("click", button_2, () => set(selected, null));
			delegated("click", button_3, () => act(get(pick), "start"));
			delegated("click", button_4, () => act(get(pick), "resolve", "fixed"));
			delegated("click", button_5, () => act(get(pick), "resolve", "rejected"));
			delegated("click", button_6, () => act(get(pick), "resolve", "deferred"));
			append($$anchor, aside);
		};
		if_block(node_9, ($$render) => {
			if (get(pick)) $$render(consequent_11);
		});
		reset(div_4);
		append($$anchor, div_4);
	};
	if_block(node_6, ($$render) => {
		if (!loaded()) $$render(consequent_3);
		else if (failed()) $$render(consequent_4, 1);
		else if (reports().length === 0) $$render(consequent_5, 2);
		else $$render(alternate_1, -1);
	});
	reset(div);
	bind_select_value(select, () => get(about), ($$value) => set(about, $$value));
	delegated("click", button, () => set(filing, !get(filing)));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/reports/Reports.svelte
function Reports($$anchor, $$props) {
	push($$props, true);
	let projects = prop($$props, "projects", 19, () => []);
	const reportsRead = resource(() => "/api/reports?all=true", { tell: () => "devplane report ls --all" });
	const reports = /* @__PURE__ */ user_derived(() => Array.isArray(reportsRead.data) ? reportsRead.data : []);
	const loaded = /* @__PURE__ */ user_derived(() => reportsRead.phase !== "loading");
	const failed = /* @__PURE__ */ user_derived(() => reportsRead.failure ? `${reportsRead.failure.says} — \`${reportsRead.failure.tell}\` tells more${reportsRead.data ? " (showing the last read)" : ""}` : "");
	let said = /* @__PURE__ */ state("");
	const read = () => reportsRead.reload();
	async function file(f) {
		try {
			const r = await api("/api/reports", {
				method: "POST",
				body: JSON.stringify({
					kind: f.kind,
					title: f.title,
					finding: f.words,
					evidence: {
						command: f.command || null,
						output: f.output || null
					},
					to: f.to,
					as_person: true,
					from: f.from
				})
			});
			set(said, `filed ${r.report?.id ?? ""} — ${r.says ?? ""}`);
			await read();
			return true;
		} catch (e) {
			set(said, `not filed: ${failure(e).says}`);
			return false;
		}
	}
	ReportList($$anchor, {
		get reports() {
			return get(reports);
		},
		get projects() {
			return projects();
		},
		get loaded() {
			return get(loaded);
		},
		get failed() {
			return get(failed);
		},
		get said() {
			return get(said);
		},
		file,
		reload: read
	});
	pop();
}
//#endregion
//#region src/surfaces/reports/index.ts
bind({
	surface: "global",
	combo: "g r",
	action: "go-reports",
	label: "go to the reports"
});
onAction("go-reports", () => {
	go("#reports");
	return true;
});
register({
	id: "reports",
	icon: "report",
	title: "Reports",
	heading: "Reports",
	band: "steering",
	order: 2,
	reads: ["/api/reports"],
	select: (feed) => {
		return { projects: (feed.board?.projects ?? []).map((p) => ({
			id: p.id,
			name: p.name
		})) };
	},
	component: Reports
});
//#endregion
//#region src/surfaces/review/Review.svelte
var root$11 = /* @__PURE__ */ from_html(`<h1 class="svelte-ghekux">Review<!></h1> <!>`, 1);
var root_1$11 = /* @__PURE__ */ from_html(`<div class="page svelte-ghekux"><!></div>`);
function Review($$anchor, $$props) {
	let chosen = prop($$props, "chosen", 3, ""), title = prop($$props, "title", 3, ""), review = prop($$props, "review", 3, null);
	var div = root_1$11();
	var node = child(div);
	var consequent = ($$anchor) => {
		Empty($$anchor, {
			icon: "split",
			title: "Pick a change to review",
			body: "A review reads one change against its base, in the order its project declared."
		});
	};
	var alternate = ($$anchor) => {
		var fragment_1 = root$11();
		var h1 = first_child(fragment_1);
		var node_1 = sibling(child(h1));
		var consequent_1 = ($$anchor) => {
			var text$3 = text();
			template_effect(() => set_text(text$3, `: ${title() ?? ""}`));
			append($$anchor, text$3);
		};
		if_block(node_1, ($$render) => {
			if (title()) $$render(consequent_1);
		});
		reset(h1);
		Pane(sibling(h1, 2), {
			get id() {
				return chosen();
			},
			get review() {
				return review();
			}
		});
		append($$anchor, fragment_1);
	};
	if_block(node, ($$render) => {
		if (!chosen()) $$render(consequent);
		else $$render(alternate, -1);
	});
	reset(div);
	append($$anchor, div);
}
//#endregion
//#region src/surfaces/review/index.ts
var KEYS = [
	[
		"j",
		"hunk-next",
		"next hunk"
	],
	[
		"k",
		"hunk-prev",
		"previous hunk"
	],
	[
		"n",
		"file-next",
		"next file"
	],
	[
		"p",
		"file-prev",
		"previous file"
	],
	[
		"s",
		"hunk-seen",
		"mark this hunk (in this browser, until it changes); a weakened line in it is recorded as read by the host"
	],
	[
		"f",
		"hunk-fix",
		"request a fix: a message to the change's latest run, quoting this hunk"
	],
	[
		"x",
		"hunk-expand",
		"expand this file's collapsed formatter-only hunks"
	],
	[
		"v",
		"diff-mode",
		"switch between unified and side-by-side"
	],
	[
		"1",
		"tab-risk",
		"read by risk"
	],
	[
		"2",
		"tab-intent",
		"read by intent"
	]
];
for (const scope of ["review", "change"]) for (const [combo, action, label] of KEYS) bind({
	surface: scope,
	combo,
	action,
	label
});
register({
	id: "review",
	title: "Review",
	heading: "Review",
	band: "happening",
	order: 1,
	nav: false,
	link: "review",
	tab: (feed, focus) => {
		const c = (feed.board?.changes ?? []).find((x) => x.id === focus);
		return c?.title ? `Review: ${c.title}` : "Review";
	},
	select: (feed, focus) => {
		return {
			chosen: focus,
			title: ((feed.board?.changes)?.find((x) => x.id === focus))?.title ?? ""
		};
	},
	component: Review
});
//#endregion
//#region src/surfaces/search/Search.svelte
var root$10 = /* @__PURE__ */ from_html(`<p class="fail svelte-1acjvix"> </p>`);
var root_1$10 = /* @__PURE__ */ from_html(`<a class="svelte-1acjvix"> </a>`);
var root_2$8 = /* @__PURE__ */ from_html(`<span class="t svelte-1acjvix"> </span>`);
var root_3$6 = /* @__PURE__ */ from_html(`<div class="frame svelte-1acjvix"><!></div>`);
var root_4$6 = /* @__PURE__ */ from_html(`<div class="page svelte-1acjvix"><h1 class="sr-only">Search every session</h1> <form class="bar svelte-1acjvix"><label class="field svelte-1acjvix"><!> <input type="search" aria-label="search every session" placeholder="A command, a question, an error" class="svelte-1acjvix"/></label> <button type="submit"> </button></form> <p class="note svelte-1acjvix">Tool calls, questions and errors from every session. Prompts and replies are never recorded, so they are never found.</p> <!> <!></div>`);
function Search($$anchor, $$props) {
	push($$props, true);
	let q = prop($$props, "q", 3, "");
	let query = /* @__PURE__ */ state("");
	let hits = /* @__PURE__ */ state(proxy([]));
	let ran = /* @__PURE__ */ state(false);
	let ranFor = /* @__PURE__ */ state("");
	let busy = /* @__PURE__ */ state(false);
	let said = /* @__PURE__ */ state("");
	let selected = /* @__PURE__ */ state(null);
	const address = /* @__PURE__ */ user_derived(q);
	user_effect(() => {
		const want = get(address);
		untrack(() => {
			if (want) {
				set(query, want, true);
				run();
			}
		});
	});
	let gen = 0;
	async function run() {
		const want = get(query).trim();
		if (!want) return;
		const mine = ++gen;
		set(busy, true);
		try {
			const r = await api(`/api/search?q=${encodeURIComponent(want)}`);
			if (mine !== gen) return;
			set(hits, r.hits ?? [], true);
			set(ran, true);
			set(ranFor, want, true);
			set(said, "");
		} catch (e) {
			if (mine !== gen) return;
			const f = failure(e, `devplane search ${want}`);
			set(said, `The search did not run: ${f.says}. \`${f.tell}\` tells more.`);
		} finally {
			if (mine === gen) set(busy, false);
		}
	}
	const columns = [
		{
			key: "at",
			label: "When",
			width: 150,
			sort: (h) => h.at,
			mono: true
		},
		{
			key: "run",
			label: "Session",
			width: 150,
			mono: true
		},
		{
			key: "text",
			label: "What matched"
		}
	];
	var div = root_4$6();
	var form = sibling(child(div), 2);
	var label = child(form);
	var node = child(label);
	Icon(node, {
		name: "search",
		size: 15
	});
	var input = sibling(node, 2);
	remove_input_defaults(input);
	reset(label);
	var button = sibling(label, 2);
	var text$2 = only_child(button, true);
	reset(form);
	var node_1 = sibling(form, 4);
	var consequent = ($$anchor) => {
		var p = root$10();
		var text_1 = only_child(p, true);
		template_effect(() => set_text(text_1, get(said)));
		append($$anchor, p);
	};
	if_block(node_1, ($$render) => {
		if (get(said)) $$render(consequent);
	});
	var node_2 = sibling(node_1, 2);
	var consequent_1 = ($$anchor) => {
		{
			let $0 = /* @__PURE__ */ user_derived(() => `Nothing any session recorded contains “${get(ranFor)}”.`);
			Empty($$anchor, {
				icon: "search",
				title: "Nothing matched",
				get body() {
					return get($0);
				}
			});
		}
	};
	var consequent_4 = ($$anchor) => {
		var div_1 = root_3$6();
		var node_3 = child(div_1);
		{
			const cell = ($$anchor, h = noop, c = noop) => {
				var fragment_1 = comment();
				var node_4 = first_child(fragment_1);
				var consequent_2 = ($$anchor) => {
					var text_2 = text();
					template_effect(($0) => set_text(text_2, $0), [() => new Date(h().at).toLocaleString()]);
					append($$anchor, text_2);
				};
				var consequent_3 = ($$anchor) => {
					var a = root_1$10();
					var text_3 = only_child(a, true);
					template_effect(($0, $1) => {
						set_attribute(a, "href", $0);
						set_text(text_3, $1);
					}, [() => `#why/${encodeURIComponent(h().run_id)}`, () => h().run_id.slice(0, 14)]);
					append($$anchor, a);
				};
				var alternate = ($$anchor) => {
					var span = root_2$8();
					var text_4 = only_child(span, true);
					template_effect(() => set_text(text_4, h().text));
					append($$anchor, span);
				};
				if_block(node_4, ($$render) => {
					if (c().key === "at") $$render(consequent_2);
					else if (c().key === "run") $$render(consequent_3, 1);
					else $$render(alternate, -1);
				});
				append($$anchor, fragment_1);
			};
			Grid(node_3, {
				id: "search",
				get columns() {
					return columns;
				},
				get rows() {
					return get(hits);
				},
				key: (h) => h.run_id + h.at + h.text,
				label: "matches",
				get selected() {
					return get(selected);
				},
				set selected($$value) {
					set(selected, $$value, true);
				},
				cell,
				$$slots: { cell: true }
			});
		}
		reset(div_1);
		append($$anchor, div_1);
	};
	if_block(node_2, ($$render) => {
		if (get(ran) && get(hits).length === 0 && !get(busy)) $$render(consequent_1);
		else if (get(hits).length > 0) $$render(consequent_4, 1);
	});
	reset(div);
	template_effect(() => {
		button.disabled = get(busy);
		set_text(text$2, get(busy) ? "Searching…" : "Search");
	});
	event("submit", form, (e) => {
		e.preventDefault();
		run();
	});
	bind_value(input, () => get(query), ($$value) => set(query, $$value));
	append($$anchor, div);
	pop();
}
//#endregion
//#region src/surfaces/search/index.ts
register({
	id: "search",
	icon: "search",
	title: "Search",
	heading: "Search every session",
	band: "happening",
	order: 2,
	nav: false,
	takesQuery: true,
	reads: ["/api/search"],
	select: (_feed, focus) => ({ q: focus }),
	component: Search
});
//#endregion
//#region src/surfaces/setup/GitHubPanel.svelte
function hostSays(h) {
	switch (h.state) {
		case "signed_in": return `signed in as ${h.login ?? "?"}${h.scopes?.length ? ` · ${h.scopes.join(", ")}` : ""}`;
		case "pending": return "waiting for the code to be entered at GitHub";
		case "expired": return "sign-in expired — GitHub no longer accepts the token, so it was removed";
		case "rate_limited": return `signed in; the rate limit is spent until ${h.until ?? "GitHub's reset"}`;
		case "unreachable": return `signed in; GitHub unreachable${h.why ? ` — ${h.why}` : ""}`;
		default: return "not signed in";
	}
}
var root$9 = /* @__PURE__ */ from_html(`<button>Sign out</button>`);
var root_1$9 = /* @__PURE__ */ from_html(`<button>Sign in</button>`);
var root_2$7 = /* @__PURE__ */ from_html(`<p class="said svelte-t93vvx"> </p>`);
var root_3$5 = /* @__PURE__ */ from_html(`<p class="quiet svelte-t93vvx"> <code class="svelte-t93vvx"> </code> signs in with GitHub CLI's token, or pipe a fine-grained personal access token to the same command.</p>`);
var root_4$5 = /* @__PURE__ */ from_html(`<div class="code svelte-t93vvx"><p class="svelte-t93vvx">Open <a target="_blank" rel="noopener noreferrer"> </a> and enter:</p> <p class="big svelte-t93vvx"> </p> <button>Copy code</button></div>`);
var root_5$4 = /* @__PURE__ */ from_html(`<div class="host svelte-t93vvx"><div class="line svelte-t93vvx"><b> </b> <span> </span> <!></div> <!> <!> <!></div>`);
var root_6$4 = /* @__PURE__ */ from_html(`<p class="quiet svelte-t93vvx">Reading the sign-in…</p>`);
var root_7$4 = /* @__PURE__ */ from_html(`<p class="quiet svelte-t93vvx"> </p>`);
var root_8$4 = /* @__PURE__ */ from_html(`<section class="card wide svelte-t93vvx" id="github"><h2 class="svelte-t93vvx"><!> GitHub</h2> <!> <!> <!> <p class="quiet svelte-t93vvx">The token is kept only in this machine's credential store — never in a file, a log or this page.</p></section>`);
function GitHubPanel($$anchor, $$props) {
	push($$props, true);
	let hosts = prop($$props, "hosts", 19, () => []), clientId = prop($$props, "clientId", 3, true), busy = prop($$props, "busy", 3, ""), said = prop($$props, "said", 3, ""), signIn = prop($$props, "signIn", 3, () => {}), signOut = prop($$props, "signOut", 3, () => {}), copy = prop($$props, "copy", 3, () => {});
	var section = root_8$4();
	var h2 = child(section);
	Icon(child(h2), {
		name: "forge",
		size: 14
	});
	next();
	reset(h2);
	var node_1 = sibling(h2, 2);
	each(node_1, 17, hosts, (h) => h.host, ($$anchor, h) => {
		var div = root_5$4();
		var div_1 = child(div);
		var b = child(div_1);
		var text = only_child(b, true);
		var span = sibling(b, 2);
		let classes;
		var text_1 = only_child(span, true);
		var node_2 = sibling(span, 2);
		var consequent = ($$anchor) => {
			var button = root$9();
			template_effect(() => button.disabled = !!busy());
			delegated("click", button, () => signOut()(get(h).host));
			append($$anchor, button);
		};
		var consequent_1 = ($$anchor) => {
			var button_1 = root_1$9();
			template_effect(() => button_1.disabled = !!busy() || !(get(h).device_flow ?? clientId()));
			delegated("click", button_1, () => signIn()(get(h).host));
			append($$anchor, button_1);
		};
		if_block(node_2, ($$render) => {
			if (get(h).state === "signed_in" || get(h).state === "rate_limited" || get(h).state === "unreachable") $$render(consequent);
			else if (get(h).state !== "pending") $$render(consequent_1, 1);
		});
		reset(div_1);
		var node_3 = sibling(div_1, 2);
		var consequent_2 = ($$anchor) => {
			var p = root_2$7();
			var text_2 = only_child(p, true);
			template_effect(() => set_text(text_2, get(h).said));
			append($$anchor, p);
		};
		if_block(node_3, ($$render) => {
			if (get(h).said) $$render(consequent_2);
		});
		var node_4 = sibling(node_3, 2);
		var consequent_3 = ($$anchor) => {
			var p_1 = root_3$5();
			var text_3 = child(p_1);
			var text_4 = only_child(sibling(text_3));
			next();
			reset(p_1);
			template_effect(() => {
				set_text(text_3, `No GitHub app is registered for ${get(h).host ?? ""}, so the window cannot start a sign-in. In a terminal, `);
				set_text(text_4, `gh auth token | devplane login github --with-token${get(h).host === "github.com" ? "" : ` --host ${get(h).host}`}`);
			});
			append($$anchor, p_1);
		};
		if_block(node_4, ($$render) => {
			if (!(get(h).device_flow ?? clientId()) && get(h).state !== "signed_in") $$render(consequent_3);
		});
		var node_5 = sibling(node_4, 2);
		var consequent_4 = ($$anchor) => {
			var div_2 = root_4$5();
			var p_2 = child(div_2);
			var a = sibling(child(p_2));
			var text_5 = only_child(a, true);
			next();
			reset(p_2);
			var p_3 = sibling(p_2, 2);
			var text_6 = only_child(p_3, true);
			var button_2 = sibling(p_3, 2);
			reset(div_2);
			template_effect(($0) => {
				set_attribute(a, "href", $0);
				set_text(text_5, get(h).verification_uri);
				set_text(text_6, get(h).user_code);
			}, [() => safeHref(get(h).verification_uri) ?? "#setup"]);
			delegated("click", button_2, () => copy()(get(h).user_code ?? ""));
			append($$anchor, div_2);
		};
		if_block(node_5, ($$render) => {
			if (get(h).state === "pending" && get(h).user_code) $$render(consequent_4);
		});
		reset(div);
		template_effect(($0) => {
			set_attribute(div, "data-state", get(h).state);
			set_text(text, get(h).host);
			classes = set_class(span, 1, "svelte-t93vvx", null, classes, {
				ok: get(h).state === "signed_in",
				warn: get(h).state !== "signed_in"
			});
			set_text(text_1, $0);
		}, [() => hostSays(get(h))]);
		append($$anchor, div);
	}, ($$anchor) => {
		append($$anchor, root_6$4());
	});
	var node_6 = sibling(node_1, 2);
	var consequent_5 = ($$anchor) => {
		var p_5 = root_7$4();
		var text_7 = only_child(p_5);
		template_effect(() => set_text(text_7, `${busy() ?? ""}…`));
		append($$anchor, p_5);
	};
	if_block(node_6, ($$render) => {
		if (busy()) $$render(consequent_5);
	});
	var node_7 = sibling(node_6, 2);
	var consequent_6 = ($$anchor) => {
		var p_6 = root_2$7();
		var text_8 = only_child(p_6, true);
		template_effect(() => set_text(text_8, said()));
		append($$anchor, p_6);
	};
	if_block(node_7, ($$render) => {
		if (said()) $$render(consequent_6);
	});
	next(2);
	reset(section);
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/setup/Setup.svelte
var root$8 = /* @__PURE__ */ from_html(`<p class="quiet svelte-1y95tkv">Reading the files…</p>`);
var root_1$8 = /* @__PURE__ */ from_html(`<p class="quiet svelte-1y95tkv">No agent settings file was found. <code class="svelte-1y95tkv">devplane connect claude</code> writes the hooks, and <code class="svelte-1y95tkv">devplane disconnect claude</code> removes exactly what it wrote.</p>`);
var root_2$6 = /* @__PURE__ */ from_html(`<p class="quiet svelte-1y95tkv">No <code class="svelte-1y95tkv">~/.devplane/policy.toml</code>. Rules live in each project's <code class="svelte-1y95tkv">devplane.toml</code>; a rule you want everywhere goes in that file, in the same shape.</p>`);
var root_3$4 = /* @__PURE__ */ from_html(`<p class="fail svelte-1y95tkv"><!> </p>`);
var root_4$4 = /* @__PURE__ */ from_html(`<code class="rule deny svelte-1y95tkv"> </code>`);
var root_5$3 = /* @__PURE__ */ from_html(`<span class="quiet svelte-1y95tkv">none</span>`);
var root_6$3 = /* @__PURE__ */ from_html(`<code class="rule ask svelte-1y95tkv"> </code>`);
var root_7$3 = /* @__PURE__ */ from_html(`<p class="file svelte-1y95tkv"><code class="svelte-1y95tkv"> </code></p> <div class="rules svelte-1y95tkv"><div class="svelte-1y95tkv"><h3 class="svelte-1y95tkv">Never</h3> <!></div> <div class="svelte-1y95tkv"><h3 class="svelte-1y95tkv">Always ask</h3> <!></div></div>`, 1);
var root_8$3 = /* @__PURE__ */ from_html(`<p class="quiet svelte-1y95tkv">None registered. A project is registered the first time an agent works in it, or with <code class="svelte-1y95tkv">devplane trust &lt;path&gt;</code>.</p>`);
var root_9$2 = /* @__PURE__ */ from_html(`<b> </b>`);
var root_10$2 = /* @__PURE__ */ from_html(`<span class="fail svelte-1y95tkv">unreadable</span>`);
var root_11$1 = /* @__PURE__ */ from_html(`<span class="ok svelte-1y95tkv">declared</span>`);
var root_12 = /* @__PURE__ */ from_html(`<span class="quiet svelte-1y95tkv">none declared</span>`);
var root_13 = /* @__PURE__ */ from_html(`<div class="frame svelte-1y95tkv"><!></div> <!>`, 1);
var root_14 = /* @__PURE__ */ from_html(`<div class="grid svelte-1y95tkv"><section class="card svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> This machine</h2> <!></section> <section class="card svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> Your agent</h2> <!></section> <!> <section class="card wide svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> Rules for every project</h2> <!></section></div> <section class="projects svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> Projects <span class="svelte-1y95tkv"> </span></h2> <!></section>`, 1);
var root_15 = /* @__PURE__ */ from_html(`<div class="page svelte-1y95tkv"><h1 class="svelte-1y95tkv">Setup</h1> <!></div>`);
function Setup($$anchor, $$props) {
	push($$props, true);
	const read = resource(() => "/api/setup", { tell: () => "devplane doctor" });
	const github = resource(() => "/api/github", {
		every: 3e3,
		tell: () => "devplane doctor"
	});
	let busy = /* @__PURE__ */ state("");
	let said = /* @__PURE__ */ state("");
	async function signIn(host) {
		if (get(busy)) return;
		set(busy, `starting a sign-in to ${host}`);
		set(said, "");
		try {
			await api("/api/github/login", {
				method: "POST",
				body: JSON.stringify({ host })
			});
		} catch (e) {
			set(said, e instanceof Error ? e.message : String(e), true);
		} finally {
			set(busy, "");
			await github.reload();
		}
	}
	async function signOut(host) {
		if (get(busy)) return;
		set(busy, `signing out of ${host}`);
		set(said, "");
		try {
			const r = await api("/api/github/logout", {
				method: "POST",
				body: JSON.stringify({ host })
			});
			set(said, r.deleted_from ? `Signed out of ${host}: the token is deleted from ${r.deleted_from}. The grant still exists at GitHub; revoke it at ${r.revoke_at ?? "GitHub's settings"}.` : `Not signed in to ${host}; there was no token to delete.`, true);
		} catch (e) {
			set(said, e instanceof Error ? e.message : String(e), true);
		} finally {
			set(busy, "");
			await github.reload();
		}
	}
	async function copy(code) {
		set(said, await copyText(code) ? `copied ${code}` : "this page cannot reach the clipboard — type the code as shown", true);
	}
	const data = /* @__PURE__ */ user_derived(() => read.data);
	const hooks = /* @__PURE__ */ user_derived(() => get(data)?.connect?.hooks_installed ?? []);
	const policy = /* @__PURE__ */ user_derived(() => get(data)?.machine_policy ?? null);
	let selected = /* @__PURE__ */ state(null);
	const columns = [
		{
			key: "name",
			label: "Project",
			width: 180,
			sort: (r) => r.name
		},
		{
			key: "trusted",
			label: "Trusted",
			width: 110,
			sort: (r) => r.trusted ? 0 : 1
		},
		{
			key: "gates",
			label: "Gates",
			width: 130,
			sort: (r) => r.declares_gates ? 0 : 1
		},
		{
			key: "config",
			label: "devplane.toml",
			width: 150
		},
		{
			key: "root",
			label: "Where",
			mono: true
		}
	];
	var div = root_15();
	var node = sibling(child(div), 2);
	var consequent = ($$anchor) => {
		Failed($$anchor, {
			what: "what is configured",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_1 = ($$anchor) => {
		append($$anchor, root$8());
	};
	var alternate_6 = ($$anchor) => {
		var fragment_1 = root_14();
		var div_1 = first_child(fragment_1);
		var section = child(div_1);
		var h2 = child(section);
		Icon(child(h2), {
			name: "settings",
			size: 14
		});
		next();
		reset(h2);
		var node_2 = sibling(h2, 2);
		{
			let $0 = /* @__PURE__ */ user_derived(() => [
				{
					label: "Version",
					value: get(data).machine?.version ?? null,
					missing: "unknown"
				},
				{
					label: "Home",
					value: get(data).machine?.home ?? null,
					mono: true
				},
				{
					label: "Record",
					value: get(data).machine?.database ?? null,
					mono: true
				},
				{
					label: "Sessions from",
					value: get(data).provider?.name ? `${get(data).provider.name}${get(data).provider.because ? ` — ${get(data).provider.because}` : ""}` : null
				}
			]);
			Props(node_2, { get rows() {
				return get($0);
			} });
		}
		reset(section);
		var section_1 = sibling(section, 2);
		var h2_1 = child(section_1);
		Icon(child(h2_1), {
			name: "agent",
			size: 14
		});
		next();
		reset(h2_1);
		var node_4 = sibling(h2_1, 2);
		var consequent_2 = ($$anchor) => {
			append($$anchor, root_1$8());
		};
		var alternate = ($$anchor) => {
			{
				let $0 = /* @__PURE__ */ user_derived(() => [
					{
						label: "Settings",
						value: get(data).connect.settings_path ?? null,
						mono: true
					},
					{
						label: "Hooks",
						value: get(hooks).length ? get(hooks).join(", ") : null,
						missing: "none installed — devplane connect claude"
					},
					{
						label: "Telemetry",
						value: !get(data).connect.telemetry_endpoint ? "not exporting" : get(data).connect.telemetry_is_ours ? "to Devplane" : "to somebody else's collector — left alone"
					}
				]);
				Props($$anchor, { get rows() {
					return get($0);
				} });
			}
		};
		if_block(node_4, ($$render) => {
			if (!get(data).connect) $$render(consequent_2);
			else $$render(alternate, -1);
		});
		reset(section_1);
		var node_5 = sibling(section_1, 2);
		var consequent_3 = ($$anchor) => {
			Failed($$anchor, {
				what: "the GitHub sign-in",
				get failure() {
					return github.failure;
				}
			});
		};
		var alternate_1 = ($$anchor) => {
			{
				let $0 = /* @__PURE__ */ user_derived(() => github.data?.hosts ?? []);
				let $1 = /* @__PURE__ */ user_derived(() => github.data?.client_id ?? true);
				GitHubPanel($$anchor, {
					get hosts() {
						return get($0);
					},
					get clientId() {
						return get($1);
					},
					get busy() {
						return get(busy);
					},
					get said() {
						return get(said);
					},
					signIn,
					signOut,
					copy
				});
			}
		};
		if_block(node_5, ($$render) => {
			if (github.phase === "failed" && github.failure) $$render(consequent_3);
			else $$render(alternate_1, -1);
		});
		var section_2 = sibling(node_5, 2);
		var h2_2 = child(section_2);
		Icon(child(h2_2), {
			name: "shield",
			size: 14
		});
		next();
		reset(h2_2);
		var node_7 = sibling(h2_2, 2);
		var consequent_4 = ($$anchor) => {
			append($$anchor, root_2$6());
		};
		var consequent_5 = ($$anchor) => {
			var p_4 = root_3$4();
			var node_8 = child(p_4);
			Icon(node_8, {
				name: "alert",
				size: 13
			});
			var text = sibling(node_8);
			reset(p_4);
			template_effect(() => set_text(text, ` ${get(policy).path ?? ""} will not parse — every gated call asks until it does: ${get(policy).error ?? ""}`));
			append($$anchor, p_4);
		};
		var alternate_2 = ($$anchor) => {
			var fragment_5 = root_7$3();
			var p_5 = first_child(fragment_5);
			var text_1 = only_child(child(p_5), true);
			reset(p_5);
			var div_2 = sibling(p_5, 2);
			var div_3 = child(div_2);
			each(sibling(child(div_3), 2), 16, () => get(policy).deny ?? [], (r) => r, ($$anchor, r) => {
				var code_2 = root_4$4();
				var text_2 = only_child(code_2, true);
				template_effect(() => set_text(text_2, r));
				append($$anchor, code_2);
			}, ($$anchor) => {
				append($$anchor, root_5$3());
			});
			reset(div_3);
			var div_4 = sibling(div_3, 2);
			each(sibling(child(div_4), 2), 16, () => get(policy).ask ?? [], (r) => r, ($$anchor, r) => {
				var code_3 = root_6$3();
				var text_3 = only_child(code_3, true);
				template_effect(() => set_text(text_3, r));
				append($$anchor, code_3);
			}, ($$anchor) => {
				append($$anchor, root_5$3());
			});
			reset(div_4);
			reset(div_2);
			template_effect(() => set_text(text_1, get(policy).path));
			append($$anchor, fragment_5);
		};
		if_block(node_7, ($$render) => {
			if (!get(policy) || !get(policy).exists) $$render(consequent_4);
			else if (get(policy).error) $$render(consequent_5, 1);
			else $$render(alternate_2, -1);
		});
		reset(section_2);
		reset(div_1);
		var section_3 = sibling(div_1, 2);
		var h2_3 = child(section_3);
		var node_11 = child(h2_3);
		Icon(node_11, {
			name: "folder",
			size: 14
		});
		var text_4 = only_child(sibling(node_11, 2), true);
		reset(h2_3);
		var node_12 = sibling(h2_3, 2);
		var consequent_6 = ($$anchor) => {
			append($$anchor, root_8$3());
		};
		var alternate_5 = ($$anchor) => {
			var fragment_6 = root_13();
			var div_5 = first_child(fragment_6);
			var node_13 = child(div_5);
			{
				const cell = ($$anchor, r = noop, c = noop) => {
					var fragment_7 = comment();
					var node_14 = first_child(fragment_7);
					var consequent_7 = ($$anchor) => {
						var b = root_9$2();
						var text_5 = only_child(b, true);
						template_effect(() => set_text(text_5, r().name));
						append($$anchor, b);
					};
					var consequent_8 = ($$anchor) => {
						{
							let $0 = /* @__PURE__ */ user_derived(() => r().trusted ? "trusted" : "not trusted");
							let $1 = /* @__PURE__ */ user_derived(() => r().trusted ? "none" : "wait");
							Pill($$anchor, {
								get word() {
									return get($0);
								},
								get as() {
									return get($1);
								}
							});
						}
					};
					var consequent_11 = ($$anchor) => {
						var fragment_9 = comment();
						var node_15 = first_child(fragment_9);
						var consequent_9 = ($$anchor) => {
							append($$anchor, root_10$2());
						};
						var consequent_10 = ($$anchor) => {
							append($$anchor, root_11$1());
						};
						var alternate_3 = ($$anchor) => {
							append($$anchor, root_12());
						};
						if_block(node_15, ($$render) => {
							if (r().error) $$render(consequent_9);
							else if (r().declares_gates) $$render(consequent_10, 1);
							else $$render(alternate_3, -1);
						});
						append($$anchor, fragment_9);
					};
					var consequent_12 = ($$anchor) => {
						var text_6 = text();
						template_effect(() => set_text(text_6, r().config?.exists ? "present" : "absent"));
						append($$anchor, text_6);
					};
					var alternate_4 = ($$anchor) => {
						var text_7 = text();
						template_effect(() => set_text(text_7, r().root));
						append($$anchor, text_7);
					};
					if_block(node_14, ($$render) => {
						if (c().key === "name") $$render(consequent_7);
						else if (c().key === "trusted") $$render(consequent_8, 1);
						else if (c().key === "gates") $$render(consequent_11, 2);
						else if (c().key === "config") $$render(consequent_12, 3);
						else $$render(alternate_4, -1);
					});
					append($$anchor, fragment_7);
				};
				let $0 = /* @__PURE__ */ user_derived(() => get(data).projects ?? []);
				Grid(node_13, {
					id: "setup-projects",
					get columns() {
						return columns;
					},
					get rows() {
						return get($0);
					},
					key: (r) => r.id,
					label: "projects",
					dense: true,
					get selected() {
						return get(selected);
					},
					set selected($$value) {
						set(selected, $$value, true);
					},
					cell,
					$$slots: { cell: true }
				});
			}
			reset(div_5);
			each(sibling(div_5, 2), 17, () => (get(data).projects ?? []).filter((p) => p.error), (p) => p.id, ($$anchor, p) => {
				var p_7 = root_3$4();
				var node_17 = child(p_7);
				Icon(node_17, {
					name: "alert",
					size: 13
				});
				var text_8 = sibling(node_17);
				reset(p_7);
				template_effect(() => set_text(text_8, ` ${get(p).name ?? ""}: ${get(p).error ?? ""}`));
				append($$anchor, p_7);
			});
			append($$anchor, fragment_6);
		};
		if_block(node_12, ($$render) => {
			if ((get(data).projects ?? []).length === 0) $$render(consequent_6);
			else $$render(alternate_5, -1);
		});
		reset(section_3);
		template_effect(() => set_text(text_4, get(data).projects?.length ?? 0));
		append($$anchor, fragment_1);
	};
	if_block(node, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent);
		else if (!get(data)) $$render(consequent_1, 1);
		else $$render(alternate_6, -1);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
//#endregion
//#region src/surfaces/setup/index.ts
register({
	id: "setup",
	icon: "settings",
	title: "Setup",
	heading: "Setup",
	band: "project",
	order: 0,
	reads: ["/api/setup", "/api/github"],
	select: () => ({}),
	component: Setup
});
//#endregion
//#region src/surfaces/why/Why.svelte
var root$7 = /* @__PURE__ */ from_html(`<button> <span class="svelte-1r7h8sn"> </span></button>`);
var root_1$7 = /* @__PURE__ */ from_html(`<p class="about svelte-1r7h8sn">Narrowed to <code> </code> · <a href="#why">show everything</a></p>`);
var root_2$5 = /* @__PURE__ */ from_html(`<p class="more svelte-1r7h8sn"> <code> </code> has every one.</p>`);
var root_3$3 = /* @__PURE__ */ from_html(`<span> </span>`);
var root_4$3 = /* @__PURE__ */ from_html(`<span class="dim svelte-1r7h8sn"> </span>`);
var root_5$2 = /* @__PURE__ */ from_html(`<p class="quiet svelte-1r7h8sn"> </p>`);
var root_6$2 = /* @__PURE__ */ from_html(`<p> </p>`);
var root_7$2 = /* @__PURE__ */ from_html(`<a class="svelte-1r7h8sn"><!> the change</a>`);
var root_8$2 = /* @__PURE__ */ from_html(`<a class="svelte-1r7h8sn"><!> everything about this run</a>`);
var root_9$1 = /* @__PURE__ */ from_html(`<aside class="drawer svelte-1r7h8sn"><header class="svelte-1r7h8sn"><!> <b> </b> <button class="x svelte-1r7h8sn" aria-label="close"><!></button></header> <p class="when svelte-1r7h8sn"> </p> <pre class="svelte-1r7h8sn"> </pre> <!> <div class="links svelte-1r7h8sn"><!> <!></div></aside>`);
var root_10$1 = /* @__PURE__ */ from_html(`<div class="frame svelte-1r7h8sn"><!> <!></div>`);
var root_11 = /* @__PURE__ */ from_html(`<div class="ledger svelte-1r7h8sn"><header class="head svelte-1r7h8sn"><h1 class="svelte-1r7h8sn">Ledger</h1> <button title="rule, timer and nobody — what was decided instead of you"><!> Decided for you <span class="svelte-1r7h8sn"> </span></button> <div class="chips svelte-1r7h8sn" role="group" aria-label="by authority"></div> <span class="gap svelte-1r7h8sn"></span> <label class="find svelte-1r7h8sn"><!><input placeholder="Filter" aria-label="filter the ledger" class="svelte-1r7h8sn"/></label></header> <!> <!> <!> <!></div>`);
function Why($$anchor, $$props) {
	push($$props, true);
	let about = prop($$props, "about", 3, "");
	const LIMIT = 1e3;
	const want = /* @__PURE__ */ user_derived(about);
	const read = resource(() => get(want) ? `/api/decisions?about=${encodeURIComponent(get(want))}&limit=${LIMIT}` : `/api/decisions?limit=${LIMIT}`, { tell: () => get(want) ? `devplane audit ${get(want)}` : "devplane audit" });
	const rows = /* @__PURE__ */ user_derived(() => Array.isArray(read.data) ? read.data : null);
	const AUTH = [
		"person",
		"rule",
		"timer",
		"nobody",
		"devplane"
	];
	const FOR_YOU = /* @__PURE__ */ new Set([
		"rule",
		"timer",
		"nobody"
	]);
	let only = /* @__PURE__ */ state(null);
	let forYou = /* @__PURE__ */ state(false);
	let text$1 = /* @__PURE__ */ state("");
	let selected = /* @__PURE__ */ state(null);
	const counts = /* @__PURE__ */ user_derived(() => AUTH.map((a) => [a, (get(rows) ?? []).filter((r) => r.authority === a).length]));
	const shown = /* @__PURE__ */ user_derived(() => (get(rows) ?? []).filter((r) => (!get(only) || r.authority === get(only)) && (!get(forYou) || FOR_YOU.has(r.authority)) && (!get(text$1).trim() || `${r.action} ${r.subject} ${r.reason ?? ""} ${r.outcome}`.toLowerCase().includes(get(text$1).trim().toLowerCase()))));
	const pick = /* @__PURE__ */ user_derived(() => get(shown).find((r) => r.id === get(selected)) ?? null);
	const tone = (a) => a === "person" ? "work" : a === "rule" ? "wait" : a === "devplane" ? "none" : "fail";
	const day = (r) => new Date(r.at).toLocaleDateString(void 0, {
		weekday: "short",
		month: "short",
		day: "numeric"
	});
	const project = (p) => p ? p.split("/").filter(Boolean).pop() ?? p : "—";
	const columns = [
		{
			key: "at",
			label: "Time",
			width: 80,
			sort: (r) => r.at,
			mono: true
		},
		{
			key: "authority",
			label: "Authority",
			width: 120,
			sort: (r) => r.authority
		},
		{
			key: "outcome",
			label: "Outcome",
			width: 100,
			sort: (r) => r.outcome
		},
		{
			key: "action",
			label: "Action",
			width: 150,
			sort: (r) => r.action,
			mono: true
		},
		{
			key: "project",
			label: "Project",
			width: 110,
			sort: (r) => project(r.project_id)
		},
		{
			key: "subject",
			label: "About",
			width: 340,
			mono: true
		},
		{
			key: "reason",
			label: "Why"
		}
	];
	var div = root_11();
	var header = child(div);
	var button = sibling(child(header), 2);
	let classes;
	var node = child(button);
	Icon(node, {
		name: "person",
		size: 13
	});
	var text_1 = only_child(sibling(node, 2), true);
	reset(button);
	var div_1 = sibling(button, 2);
	each(div_1, 21, () => get(counts), ([a, n]) => a, ($$anchor, $$item) => {
		var $$array = /* @__PURE__ */ user_derived(() => to_array(get($$item), 2));
		let a = () => get($$array)[0];
		let n = () => get($$array)[1];
		var button_1 = root$7();
		let classes_1;
		var text_2 = child(button_1);
		var text_3 = only_child(sibling(text_2), true);
		reset(button_1);
		template_effect(() => {
			button_1.disabled = n() === 0;
			classes_1 = set_class(button_1, 1, "svelte-1r7h8sn", null, classes_1, { on: get(only) === a() });
			set_text(text_2, `${a() ?? ""} `);
			set_text(text_3, get(rows) ? n() : "");
		});
		delegated("click", button_1, () => set(only, get(only) === a() ? null : a(), true));
		append($$anchor, button_1);
	});
	reset(div_1);
	var label = sibling(div_1, 4);
	var node_1 = child(label);
	Icon(node_1, {
		name: "search",
		size: 13
	});
	var input = sibling(node_1);
	remove_input_defaults(input);
	reset(label);
	reset(header);
	var node_2 = sibling(header, 2);
	var consequent = ($$anchor) => {
		var p_1 = root_1$7();
		var text_4 = only_child(sibling(child(p_1)), true);
		next(2);
		reset(p_1);
		template_effect(() => set_text(text_4, about()));
		append($$anchor, p_1);
	};
	if_block(node_2, ($$render) => {
		if (about()) $$render(consequent);
	});
	var node_3 = sibling(node_2, 2);
	var consequent_1 = ($$anchor) => {
		Failed($$anchor, {
			what: "the ledger",
			get failure() {
				return read.failure;
			},
			get at() {
				return read.at;
			},
			stale: true
		});
	};
	if_block(node_3, ($$render) => {
		if (read.phase === "stale" && read.failure) $$render(consequent_1);
	});
	var node_4 = sibling(node_3, 2);
	var consequent_2 = ($$anchor) => {
		var p_2 = root_2$5();
		var text_5 = child(p_2);
		text_5.nodeValue = "The newest 1000 decisions are shown; ";
		var text_6 = only_child(sibling(text_5), true);
		next();
		reset(p_2);
		template_effect(() => set_text(text_6, get(want) ? `devplane audit ${get(want)}` : "devplane audit"));
		append($$anchor, p_2);
	};
	if_block(node_4, ($$render) => {
		if (get(rows) && get(rows).length >= LIMIT) $$render(consequent_2);
	});
	var node_5 = sibling(node_4, 2);
	var consequent_3 = ($$anchor) => {
		Failed($$anchor, {
			what: "the ledger",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_4 = ($$anchor) => {
		Empty($$anchor, {
			icon: "ledger",
			title: "Nothing has been decided yet",
			body: "Every refusal a rule made, every question a person answered, every gate Devplane ran — each lands here with the authority that decided it."
		});
	};
	var alternate_1 = ($$anchor) => {
		var div_2 = root_10$1();
		var node_6 = child(div_2);
		{
			const cell = ($$anchor, r = noop, c = noop) => {
				var fragment_3 = comment();
				var node_7 = first_child(fragment_3);
				var consequent_5 = ($$anchor) => {
					var text_7 = text();
					template_effect(($0) => set_text(text_7, $0), [() => new Date(r().at).toTimeString().slice(0, 5)]);
					append($$anchor, text_7);
				};
				var consequent_6 = ($$anchor) => {
					{
						let $0 = /* @__PURE__ */ user_derived(() => tone(r().authority));
						Pill($$anchor, {
							get word() {
								return r().authority;
							},
							get as() {
								return get($0);
							}
						});
					}
				};
				var consequent_7 = ($$anchor) => {
					var span_2 = root_3$3();
					var text_8 = only_child(span_2, true);
					template_effect(() => {
						set_class(span_2, 1, `out ${r().outcome ?? ""}`, "svelte-1r7h8sn");
						set_text(text_8, r().outcome);
					});
					append($$anchor, span_2);
				};
				var consequent_8 = ($$anchor) => {
					var text_9 = text();
					template_effect(() => set_text(text_9, r().action));
					append($$anchor, text_9);
				};
				var consequent_9 = ($$anchor) => {
					var text_10 = text();
					template_effect(($0) => set_text(text_10, $0), [() => project(r().project_id)]);
					append($$anchor, text_10);
				};
				var consequent_10 = ($$anchor) => {
					var text_11 = text();
					template_effect(() => set_text(text_11, r().subject));
					append($$anchor, text_11);
				};
				var alternate = ($$anchor) => {
					var span_3 = root_4$3();
					var text_12 = only_child(span_3, true);
					template_effect(() => set_text(text_12, r().reason ?? ""));
					append($$anchor, span_3);
				};
				if_block(node_7, ($$render) => {
					if (c().key === "at") $$render(consequent_5);
					else if (c().key === "authority") $$render(consequent_6, 1);
					else if (c().key === "outcome") $$render(consequent_7, 2);
					else if (c().key === "action") $$render(consequent_8, 3);
					else if (c().key === "project") $$render(consequent_9, 4);
					else if (c().key === "subject") $$render(consequent_10, 5);
					else $$render(alternate, -1);
				});
				append($$anchor, fragment_3);
			};
			const empty = ($$anchor) => {
				var p_3 = root_5$2();
				var text_13 = only_child(p_3, true);
				template_effect(() => set_text(text_13, get(rows) === null ? "Reading…" : "Nothing matches."));
				append($$anchor, p_3);
			};
			Grid(node_6, {
				id: "ledger",
				get columns() {
					return columns;
				},
				get rows() {
					return get(shown);
				},
				key: (r) => r.id,
				group: day,
				label: "decisions",
				get selected() {
					return get(selected);
				},
				set selected($$value) {
					set(selected, $$value, true);
				},
				cell,
				empty,
				$$slots: {
					cell: true,
					empty: true
				}
			});
		}
		var node_8 = sibling(node_6, 2);
		var consequent_14 = ($$anchor) => {
			var aside = root_9$1();
			var header_1 = child(aside);
			var node_9 = child(header_1);
			{
				let $0 = /* @__PURE__ */ user_derived(() => tone(get(pick).authority));
				Pill(node_9, {
					get word() {
						return get(pick).authority;
					},
					get as() {
						return get($0);
					}
				});
			}
			var b = sibling(node_9, 2);
			var text_14 = only_child(b, true);
			var button_2 = sibling(b, 2);
			Icon(child(button_2), {
				name: "x",
				size: 13
			});
			reset(button_2);
			reset(header_1);
			var p_4 = sibling(header_1, 2);
			var text_15 = only_child(p_4);
			var pre = sibling(p_4, 2);
			var text_16 = only_child(pre, true);
			var node_11 = sibling(pre, 2);
			var consequent_11 = ($$anchor) => {
				var p_5 = root_6$2();
				var text_17 = only_child(p_5, true);
				template_effect(() => set_text(text_17, get(pick).reason));
				append($$anchor, p_5);
			};
			if_block(node_11, ($$render) => {
				if (get(pick).reason) $$render(consequent_11);
			});
			var div_3 = sibling(node_11, 2);
			var node_12 = child(div_3);
			var consequent_12 = ($$anchor) => {
				var a_1 = root_7$2();
				Icon(child(a_1), {
					name: "change",
					size: 13
				});
				next();
				reset(a_1);
				template_effect(($0) => set_attribute(a_1, "href", $0), [() => `#change/${encodeURIComponent(get(pick).change_id)}`]);
				append($$anchor, a_1);
			};
			if_block(node_12, ($$render) => {
				if (get(pick).change_id) $$render(consequent_12);
			});
			var node_14 = sibling(node_12, 2);
			var consequent_13 = ($$anchor) => {
				var a_2 = root_8$2();
				Icon(child(a_2), {
					name: "sessions",
					size: 13
				});
				next();
				reset(a_2);
				template_effect(($0) => set_attribute(a_2, "href", $0), [() => `#why/${encodeURIComponent(get(pick).run_id)}`]);
				append($$anchor, a_2);
			};
			if_block(node_14, ($$render) => {
				if (get(pick).run_id) $$render(consequent_13);
			});
			reset(div_3);
			reset(aside);
			template_effect(($0) => {
				set_text(text_14, get(pick).action);
				set_text(text_15, `${$0 ?? ""} · ${get(pick).outcome ?? ""}`);
				set_text(text_16, get(pick).subject);
			}, [() => new Date(get(pick).at).toLocaleString()]);
			delegated("click", button_2, () => set(selected, null));
			append($$anchor, aside);
		};
		if_block(node_8, ($$render) => {
			if (get(pick)) $$render(consequent_14);
		});
		reset(div_2);
		append($$anchor, div_2);
	};
	if_block(node_5, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent_3);
		else if (get(rows) && get(rows).length === 0) $$render(consequent_4, 1);
		else $$render(alternate_1, -1);
	});
	reset(div);
	template_effect(($0) => {
		classes = set_class(button, 1, "foryou svelte-1r7h8sn", null, classes, { on: get(forYou) });
		set_text(text_1, $0);
	}, [() => get(rows) ? get(rows).filter((r) => FOR_YOU.has(r.authority)).length : ""]);
	delegated("click", button, () => set(forYou, !get(forYou)));
	bind_value(input, () => get(text$1), ($$value) => set(text$1, $$value));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/why/index.ts
bind({
	surface: "global",
	combo: "g l",
	action: "go-ledger",
	label: "go to the ledger"
});
onAction("go-ledger", () => {
	go("#why");
	return true;
});
register({
	id: "why",
	icon: "ledger",
	title: "Ledger",
	heading: "Ledger",
	band: "happening",
	order: 4,
	link: "run",
	select: (_feed, focus) => ({ about: focus }),
	component: Why
});
//#endregion
//#region src/lib/theme.svelte.ts
var KEY$1 = "devplane_theme";
function read() {
	try {
		const v = localStorage.getItem(KEY$1);
		return v === "light" || v === "dark" ? v : "system";
	} catch {
		return "system";
	}
}
function theme() {
	const state = proxy({ choice: read() });
	function apply(next) {
		state.choice = next;
		if (next === "system") document.documentElement.removeAttribute("data-theme");
		else document.documentElement.setAttribute("data-theme", next);
		try {
			if (next === "system") localStorage.removeItem(KEY$1);
			else localStorage.setItem(KEY$1, next);
		} catch {}
	}
	return {
		state,
		restore: () => apply(state.choice),
		cycle: () => apply(state.choice === "system" ? "light" : state.choice === "light" ? "dark" : "system")
	};
}
//#endregion
//#region src/lib/Stale.svelte
var root$6 = /* @__PURE__ */ from_html(`<span class="age svelte-uf40wk"> </span>`);
var root_1$6 = /* @__PURE__ */ from_html(`<span class="age svelte-uf40wk">· nothing has been read yet</span>`);
var root_2$4 = /* @__PURE__ */ from_html(`<p class="stale svelte-uf40wk"><span role="status"> </span> <!></p>`);
function Stale($$anchor, $$props) {
	push($$props, true);
	let stale_since = prop($$props, "stale_since", 3, null);
	let now = /* @__PURE__ */ state(proxy(Date.now()));
	user_effect(() => {
		const id = setInterval(() => set(now, Date.now(), true), 1e3);
		return () => clearInterval(id);
	});
	const age = /* @__PURE__ */ user_derived(() => stale_since() ? ago$1(Math.max(0, (get(now) - Date.parse(stale_since())) / 1e3)) : "");
	var p = root_2$4();
	var span = child(p);
	var text = only_child(span, true);
	var node = sibling(span, 2);
	var consequent = ($$anchor) => {
		var span_1 = root$6();
		var text_1 = only_child(span_1);
		template_effect(() => set_text(text_1, `· showing what was read ${get(age) ?? ""} ago`));
		append($$anchor, span_1);
	};
	var alternate = ($$anchor) => {
		append($$anchor, root_1$6());
	};
	if_block(node, ($$render) => {
		if (get(age)) $$render(consequent);
		else $$render(alternate, -1);
	});
	reset(p);
	template_effect(() => set_text(text, $$props.error));
	append($$anchor, p);
	pop();
}
//#endregion
//#region src/lib/trap.ts
var FOCUSABLE = "a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex=\"-1\"])";
function trap(node) {
	const before = document.activeElement;
	const inside = () => [...node.querySelectorAll(FOCUSABLE)].filter((el) => el.offsetParent !== null || el === document.activeElement);
	queueMicrotask(() => {
		if (node.contains(document.activeElement)) return;
		const first = inside()[0];
		if (first) first.focus();
		else {
			if (!node.hasAttribute("tabindex")) node.setAttribute("tabindex", "-1");
			node.focus();
		}
	});
	const onKey = (e) => {
		if (e.key !== "Tab") return;
		const all = inside();
		if (all.length === 0) {
			e.preventDefault();
			return;
		}
		const first = all[0];
		const last = all[all.length - 1];
		const at = document.activeElement;
		if (e.shiftKey && (at === first || !node.contains(at))) {
			e.preventDefault();
			last.focus();
		} else if (!e.shiftKey && (at === last || !node.contains(at))) {
			e.preventDefault();
			first.focus();
		}
	};
	node.addEventListener("keydown", onKey);
	return { destroy() {
		node.removeEventListener("keydown", onKey);
		if (before && before.isConnected && typeof before.focus === "function") queueMicrotask(() => before.focus());
	} };
}
//#endregion
//#region src/shell/TitleBar.svelte
var root$5 = /* @__PURE__ */ from_html(`<!> <span> </span>`, 1);
var root_1$5 = /* @__PURE__ */ from_html(`<header class="title svelte-whzgc7"><div class="brand svelte-whzgc7"><svg class="mark svelte-whzgc7" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><rect x="2" y="2" width="20" height="20" rx="5" fill="var(--accent)"></rect><path d="M8 7h4.5a5 5 0 0 1 0 10H8z" fill="none" stroke="var(--chrome)" stroke-width="2.2"></path></svg> <b class="svelte-whzgc7">Devplane</b> <!></div> <button class="find svelte-whzgc7" aria-label="find anything — changes, sessions, commands"><!> <span class="svelte-whzgc7">Find a change, a session, a command…</span> <kbd class="svelte-whzgc7"> </kbd></button> <div class="acts svelte-whzgc7"><button class="primary svelte-whzgc7"><!> New change</button> <button aria-label="toggle the sidebar"><!></button> <button aria-label="toggle the activity panel"><!></button></div></header>`);
function TitleBar($$anchor, $$props) {
	push($$props, true);
	var header = root_1$5();
	var div = child(header);
	each(sibling(child(div), 4), 17, () => $$props.crumbs, index, ($$anchor, c, i) => {
		var fragment = root$5();
		var node_1 = first_child(fragment);
		Icon(node_1, {
			name: "right",
			size: 12
		});
		var span = sibling(node_1, 2);
		let classes;
		var text = only_child(span, true);
		template_effect(() => {
			classes = set_class(span, 1, "crumb svelte-whzgc7", null, classes, { last: i === $$props.crumbs.length - 1 });
			set_text(text, get(c));
		});
		append($$anchor, fragment);
	});
	reset(div);
	var button = sibling(div, 2);
	var node_2 = child(button);
	Icon(node_2, {
		name: "search",
		size: 14
	});
	var text_1 = only_child(sibling(node_2, 4), true);
	reset(button);
	var div_1 = sibling(button, 2);
	var button_1 = child(div_1);
	Icon(child(button_1), {
		name: "plus",
		size: 14
	});
	next();
	reset(button_1);
	var button_2 = sibling(button_1, 2);
	let classes_1;
	Icon(child(button_2), {
		name: "sidebar",
		size: 16
	});
	reset(button_2);
	var button_3 = sibling(button_2, 2);
	let classes_2;
	Icon(child(button_3), {
		name: "panel",
		size: 16
	});
	reset(button_3);
	reset(div_1);
	reset(header);
	template_effect(($0, $1, $2, $3) => {
		set_text(text_1, $0);
		set_attribute(button_1, "title", `Start a new change (${$1 ?? ""})`);
		classes_1 = set_class(button_2, 1, "icon svelte-whzgc7", null, classes_1, { on: $$props.sideOpen });
		set_attribute(button_2, "title", `Sidebar (${$2 ?? ""})`);
		set_attribute(button_2, "aria-pressed", $$props.sideOpen);
		classes_2 = set_class(button_3, 1, "icon svelte-whzgc7", null, classes_2, { on: $$props.panelOpen });
		set_attribute(button_3, "title", `Activity panel (${$3 ?? ""})`);
		set_attribute(button_3, "aria-pressed", $$props.panelOpen);
	}, [
		() => spell("Mod+k"),
		() => spell("Alt+n"),
		() => spell("Mod+b"),
		() => spell("Mod+j")
	]);
	delegated("click", button, function(...$$args) {
		$$props.find?.apply(this, $$args);
	});
	delegated("click", button_1, function(...$$args) {
		$$props.create?.apply(this, $$args);
	});
	delegated("click", button_2, function(...$$args) {
		$$props.toggleSide?.apply(this, $$args);
	});
	delegated("click", button_3, function(...$$args) {
		$$props.togglePanel?.apply(this, $$args);
	});
	append($$anchor, header);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/shell/ActivityBar.svelte
var root$4 = /* @__PURE__ */ from_html(`<b> </b>`);
var root_1$4 = /* @__PURE__ */ from_html(`<span> </span>`);
var root_2$3 = /* @__PURE__ */ from_html(`<button><!> <!></button>`);
var root_3$2 = /* @__PURE__ */ from_html(`<div class="band svelte-w2jox2" role="group"></div>`);
var root_4$2 = /* @__PURE__ */ from_html(`<nav class="bar svelte-w2jox2" aria-label="surfaces"><!> <span class="gap svelte-w2jox2"></span> <!></nav>`);
function ActivityBar($$anchor, $$props) {
	push($$props, true);
	const item = ($$anchor, s = noop) => {
		const n = /* @__PURE__ */ user_derived(() => s().count?.($$props.feed) ?? null);
		var button = root_2$3();
		let classes;
		var node = child(button);
		var consequent = ($$anchor) => {
			Icon($$anchor, {
				get name() {
					return s().icon;
				},
				size: 20,
				stroke: 1.6
			});
		};
		var alternate = ($$anchor) => {
			var b_1 = root$4();
			var text = only_child(b_1, true);
			template_effect(($0) => set_text(text, $0), [() => s().title.slice(0, 1)]);
			append($$anchor, b_1);
		};
		if_block(node, ($$render) => {
			if (s().icon) $$render(consequent);
			else $$render(alternate, -1);
		});
		var node_1 = sibling(node, 2);
		var consequent_1 = ($$anchor) => {
			var span = root_1$4();
			let classes_1;
			var text_1 = only_child(span, true);
			template_effect(() => {
				classes_1 = set_class(span, 1, "badge svelte-w2jox2", null, classes_1, { hot: s().band === "attention" });
				set_text(text_1, get(n) > 99 ? "99+" : get(n));
			});
			append($$anchor, span);
		};
		if_block(node_1, ($$render) => {
			if (get(n)) $$render(consequent_1);
		});
		reset(button);
		template_effect(() => {
			classes = set_class(button, 1, "act svelte-w2jox2", null, classes, { on: s().id === $$props.current });
			set_attribute(button, "title", get(n) ? `${s().title} — ${get(n)}` : s().title);
			set_attribute(button, "aria-label", get(n) ? `${s().title}, ${get(n)}` : s().title);
			set_attribute(button, "aria-current", s().id === $$props.current ? "page" : void 0);
		});
		delegated("click", button, () => $$props.pick(s().id));
		append($$anchor, button);
	};
	const group = ($$anchor, g = noop) => {
		var div = root_3$2();
		each(div, 21, () => g().items, (s) => s.id, ($$anchor, s) => {
			item($$anchor, () => get(s));
		});
		reset(div);
		template_effect(() => set_attribute(div, "aria-label", BAND_LABELS[g().band]));
		append($$anchor, div);
	};
	const bands = /* @__PURE__ */ user_derived(() => BANDS.map((b) => ({
		band: b,
		items: $$props.items.filter((s) => s.band === b)
	})).filter((g) => g.items.length > 0));
	const top = /* @__PURE__ */ user_derived(() => get(bands).slice(0, -1));
	const bottom = /* @__PURE__ */ user_derived(() => get(bands).slice(-1));
	var nav = root_4$2();
	var node_2 = child(nav);
	each(node_2, 17, () => get(top), (g) => g.band, ($$anchor, g) => {
		group($$anchor, () => get(g));
	});
	each(sibling(node_2, 4), 17, () => get(bottom), (g) => g.band, ($$anchor, g) => {
		group($$anchor, () => get(g));
	});
	reset(nav);
	append($$anchor, nav);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/shell/EditorTabs.svelte
var root$3 = /* @__PURE__ */ from_html(`<div role="tab"><!> <span class="name svelte-1h63k4l"> </span> <button class="x svelte-1h63k4l"><!></button></div>`);
var root_1$3 = /* @__PURE__ */ from_html(`<div class="strip svelte-1h63k4l" role="tablist" aria-label="open tabs"></div>`);
function EditorTabs($$anchor, $$props) {
	push($$props, true);
	var div = root_1$3();
	each(div, 23, () => $$props.list, (t) => t.surface + "/" + t.focus, ($$anchor, t, i) => {
		var div_1 = root$3();
		let classes;
		var node = child(div_1);
		var consequent = ($$anchor) => {
			{
				let $0 = /* @__PURE__ */ user_derived(() => $$props.icon(get(t)));
				Icon($$anchor, {
					get name() {
						return get($0);
					},
					size: 14
				});
			}
		};
		var d = /* @__PURE__ */ user_derived(() => $$props.icon(get(t)));
		if_block(node, ($$render) => {
			if (get(d)) $$render(consequent);
		});
		var span = sibling(node, 2);
		var text = only_child(span, true);
		var button = sibling(span, 2);
		Icon(child(button), {
			name: "x",
			size: 12
		});
		reset(button);
		reset(div_1);
		template_effect(($0, $1, $2) => {
			classes = set_class(div_1, 1, "tab svelte-1h63k4l", null, classes, {
				on: get(i) === $$props.active,
				preview: !get(t).pinned
			});
			set_attribute(div_1, "tabindex", get(i) === $$props.active ? 0 : -1);
			set_attribute(div_1, "aria-selected", get(i) === $$props.active);
			set_attribute(div_1, "title", $0);
			set_text(text, $1);
			set_attribute(button, "aria-label", `close ${$2 ?? ""}`);
		}, [
			() => $$props.title(get(t)),
			() => $$props.title(get(t)),
			() => $$props.title(get(t))
		]);
		delegated("click", div_1, () => $$props.pick(get(i)));
		delegated("dblclick", div_1, () => $$props.pin(get(i)));
		event("auxclick", div_1, (e) => e.button === 1 && $$props.close(get(i)));
		delegated("keydown", div_1, (e) => e.key === "Enter" && $$props.pick(get(i)));
		delegated("click", button, (e) => {
			e.stopPropagation();
			$$props.close(get(i));
		});
		append($$anchor, div_1);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
delegate([
	"click",
	"dblclick",
	"keydown"
]);
//#endregion
//#region src/shell/StatusBar.svelte
var root$2 = /* @__PURE__ */ from_html(`<button><!> </button>`);
var root_1$2 = /* @__PURE__ */ from_html(`<span class="item svelte-1xut7e1"> </span>`);
var root_2$2 = /* @__PURE__ */ from_html(`<footer class="status svelte-1xut7e1" aria-label="status"><span role="status"><span class="dot svelte-1xut7e1"></span> </span> <!> <!> <span class="gap svelte-1xut7e1"></span> <!> <button class="item svelte-1xut7e1" title="theme"><!> </button> <button class="item svelte-1xut7e1" aria-label="keys bound here"><!></button></footer>`);
function StatusBar($$anchor, $$props) {
	push($$props, true);
	const fact = ($$anchor, i = noop) => {
		var button = root$2();
		let classes;
		var node = child(button);
		var consequent = ($$anchor) => {
			Icon($$anchor, {
				get name() {
					return i().icon;
				},
				size: 13
			});
		};
		if_block(node, ($$render) => {
			if (i().icon) $$render(consequent);
		});
		var text = sibling(node);
		reset(button);
		template_effect(() => {
			classes = set_class(button, 1, `item ${i().tone ?? "" ?? ""}`, "svelte-1xut7e1", classes, { hot: i().tone === "wait" && i().n > 0 });
			set_attribute(button, "title", `open ${i().title ?? ""}`);
			set_text(text, `${i().n ?? ""} ${i().word ?? ""}`);
		});
		delegated("click", button, () => $$props.go(`#${i().surface}`));
		append($$anchor, button);
	};
	let projects = prop($$props, "projects", 3, null);
	var footer = root_2$2();
	var span = child(footer);
	let classes_1;
	var text_1 = sibling(child(span), 1, true);
	reset(span);
	var node_1 = sibling(span, 2);
	each(node_1, 17, () => $$props.items.filter((i) => !i.end), (i) => i.surface + i.word, ($$anchor, i) => {
		fact($$anchor, () => get(i));
	});
	var node_2 = sibling(node_1, 2);
	var consequent_1 = ($$anchor) => {
		var span_1 = root_1$2();
		var text_2 = only_child(span_1);
		template_effect(() => set_text(text_2, `${projects() ?? ""} projects`));
		append($$anchor, span_1);
	};
	if_block(node_2, ($$render) => {
		if (projects() != null) $$render(consequent_1);
	});
	var node_3 = sibling(node_2, 4);
	each(node_3, 17, () => $$props.items.filter((i) => i.end), (i) => i.surface + i.word, ($$anchor, i) => {
		fact($$anchor, () => get(i));
	});
	var button_1 = sibling(node_3, 2);
	var node_4 = child(button_1);
	{
		let $0 = /* @__PURE__ */ user_derived(() => $$props.theme === "light" ? "sun" : "moon");
		Icon(node_4, {
			get name() {
				return get($0);
			},
			size: 13
		});
	}
	var text_3 = sibling(node_4, 1, true);
	reset(button_1);
	var button_2 = sibling(button_1, 2);
	Icon(child(button_2), {
		name: "keyboard",
		size: 13
	});
	reset(button_2);
	reset(footer);
	template_effect(() => {
		classes_1 = set_class(span, 1, "item pulse svelte-1xut7e1", null, classes_1, { bad: $$props.bad });
		set_text(text_1, $$props.pulse);
		set_text(text_3, $$props.theme);
	});
	delegated("click", button_1, function(...$$args) {
		$$props.cycleTheme?.apply(this, $$args);
	});
	delegated("click", button_2, function(...$$args) {
		$$props.help?.apply(this, $$args);
	});
	append($$anchor, footer);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/shell/Panel.svelte
var root$1 = /* @__PURE__ */ from_html(`<p class="quiet fail svelte-9fbpju"> </p>`);
var root_1$1 = /* @__PURE__ */ from_html(`<p class="quiet svelte-9fbpju">Reading the sessions…</p>`);
var root_2$1 = /* @__PURE__ */ from_html(`<p class="quiet svelte-9fbpju">No session has reported anything yet.</p>`);
var root_3$1 = /* @__PURE__ */ from_html(`<span class="ctx svelte-9fbpju"> </span>`);
var root_4$1 = /* @__PURE__ */ from_html(`<li><button class="svelte-9fbpju"><time class="svelte-9fbpju"> </time> <span class="proj svelte-9fbpju"> </span> <span class="agent svelte-9fbpju"> </span> <!> <span class="what svelte-9fbpju"> </span> <!></button></li>`);
var root_5$1 = /* @__PURE__ */ from_html(`<ol class="log svelte-9fbpju"></ol>`);
var root_6$1 = /* @__PURE__ */ from_html(`<dt class="svelte-9fbpju"><!> Projects whose configuration cannot be read</dt> <dd class="svelte-9fbpju"> </dd>`, 1);
var root_7$1 = /* @__PURE__ */ from_html(`<dl class="sight svelte-9fbpju"><dt class="svelte-9fbpju"><!> Watched end to end</dt> <dd class="svelte-9fbpju"> </dd> <dt class="svelte-9fbpju"><!> Read, not yet proved against a live session</dt> <dd class="svelte-9fbpju"> </dd> <dt class="svelte-9fbpju"><!> Seen only when Devplane starts them</dt> <dd class="svelte-9fbpju"> </dd> <!></dl> <p class="quiet svelte-9fbpju">A session in a tool that is not watched here is not on the board — the board is the limit of this machine's sight, not a report that nothing is running.</p>`, 1);
var root_8$1 = /* @__PURE__ */ from_html(`<section class="panel svelte-9fbpju" aria-label="activity panel"><div class="head svelte-9fbpju" role="tablist" aria-label="panel views"><button role="tab" class="svelte-9fbpju">Activity <span class="n svelte-9fbpju"> </span></button> <button role="tab" class="svelte-9fbpju">Sight <span class="n svelte-9fbpju"> </span></button></div> <div class="body svelte-9fbpju"><!></div></section>`);
function Panel($$anchor, $$props) {
	push($$props, true);
	let error = prop($$props, "error", 3, null);
	let view = /* @__PURE__ */ state("activity");
	const runs = /* @__PURE__ */ user_derived(() => [...$$props.board?.runs ?? []].sort((a, b) => (b.last_event_at ?? "").localeCompare(a.last_event_at ?? "")));
	const w = /* @__PURE__ */ user_derived(() => $$props.board?.watching ?? {});
	function clock(at) {
		if (!at) return "--:--:--";
		const d = new Date(at);
		return Number.isNaN(d.getTime()) ? "--:--:--" : d.toTimeString().slice(0, 8);
	}
	var section = root_8$1();
	var div = child(section);
	var button = child(div);
	var text = only_child(sibling(child(button)), true);
	reset(button);
	var button_1 = sibling(button, 2);
	var text_1 = only_child(sibling(child(button_1)), true);
	reset(button_1);
	reset(div);
	var div_1 = sibling(div, 2);
	var node = child(div_1);
	var consequent_4 = ($$anchor) => {
		var fragment = comment();
		var node_1 = first_child(fragment);
		var consequent = ($$anchor) => {
			var p = root$1();
			var text_2 = only_child(p);
			template_effect(() => set_text(text_2, `The sessions could not be read: ${error() ?? ""}`));
			append($$anchor, p);
		};
		var consequent_1 = ($$anchor) => {
			append($$anchor, root_1$1());
		};
		var consequent_2 = ($$anchor) => {
			append($$anchor, root_2$1());
		};
		var alternate = ($$anchor) => {
			var ol = root_5$1();
			each(ol, 21, () => get(runs), (r) => r.id, ($$anchor, r) => {
				var li = root_4$1();
				var button_2 = child(li);
				var time = child(button_2);
				var text_3 = only_child(time, true);
				var span_2 = sibling(time, 2);
				var text_4 = only_child(span_2, true);
				var span_3 = sibling(span_2, 2);
				var text_5 = only_child(span_3);
				var node_2 = sibling(span_3, 2);
				{
					let $0 = /* @__PURE__ */ user_derived(() => get(r).state ?? "unknown");
					Pill(node_2, { get word() {
						return get($0);
					} });
				}
				var span_4 = sibling(node_2, 2);
				var text_6 = only_child(span_4, true);
				var node_3 = sibling(span_4, 2);
				var consequent_3 = ($$anchor) => {
					var span_5 = root_3$1();
					var text_7 = only_child(span_5);
					template_effect(($0) => set_text(text_7, `${$0 ?? ""}% context`), [() => Math.round(get(r).context_percent)]);
					append($$anchor, span_5);
				};
				if_block(node_3, ($$render) => {
					if (get(r).context_percent != null) $$render(consequent_3);
				});
				reset(button_2);
				reset(li);
				template_effect(($0) => {
					button_2.disabled = !$$props.open;
					set_text(text_3, $0);
					set_text(text_4, get(r).project_name ?? "—");
					set_text(text_5, `${get(r).agent ?? "agent" ?? ""}${get(r).mode === "driven" ? " · driven" : ""}`);
					set_text(text_6, get(r).summary ?? "");
				}, [() => clock(get(r).last_event_at)]);
				delegated("click", button_2, () => $$props.open?.(get(r).id));
				append($$anchor, li);
			});
			reset(ol);
			append($$anchor, ol);
		};
		if_block(node_1, ($$render) => {
			if (!$$props.board && error()) $$render(consequent);
			else if (!$$props.board) $$render(consequent_1, 1);
			else if (get(runs).length === 0) $$render(consequent_2, 2);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_1 = root_7$1();
		var dl = first_child(fragment_1);
		var dt = child(dl);
		Icon(child(dt), {
			name: "eye",
			size: 13
		});
		next();
		reset(dt);
		var dd = sibling(dt, 2);
		var text_8 = only_child(dd, true);
		var dt_1 = sibling(dd, 2);
		Icon(child(dt_1), {
			name: "alert",
			size: 13
		});
		next();
		reset(dt_1);
		var dd_1 = sibling(dt_1, 2);
		var text_9 = only_child(dd_1, true);
		var dt_2 = sibling(dd_1, 2);
		Icon(child(dt_2), {
			name: "agent",
			size: 13
		});
		next();
		reset(dt_2);
		var dd_2 = sibling(dt_2, 2);
		var text_10 = only_child(dd_2, true);
		var node_7 = sibling(dd_2, 2);
		var consequent_5 = ($$anchor) => {
			var fragment_2 = root_6$1();
			var dt_3 = first_child(fragment_2);
			Icon(child(dt_3), {
				name: "x",
				size: 13
			});
			next();
			reset(dt_3);
			var text_11 = only_child(sibling(dt_3, 2), true);
			template_effect(($0) => set_text(text_11, $0), [() => $$props.board.coverage.unreadable.join(", ")]);
			append($$anchor, fragment_2);
		};
		if_block(node_7, ($$render) => {
			if ($$props.board?.coverage?.unreadable?.length) $$render(consequent_5);
		});
		reset(dl);
		next(2);
		template_effect(($0, $1, $2) => {
			set_text(text_8, $0);
			set_text(text_9, $1);
			set_text(text_10, $2);
		}, [
			() => get(w).watched?.join(", ") || "none",
			() => get(w).unproved?.join(", ") || "none",
			() => get(w).driven_only?.join(", ") || "none"
		]);
		append($$anchor, fragment_1);
	};
	if_block(node, ($$render) => {
		if (get(view) === "activity") $$render(consequent_4);
		else $$render(alternate_1, -1);
	});
	reset(div_1);
	reset(section);
	template_effect(() => {
		set_attribute(button, "aria-selected", get(view) === "activity");
		set_text(text, $$props.board ? get(runs).length : "");
		set_attribute(button_1, "aria-selected", get(view) === "sight");
		set_text(text_1, $$props.board ? (get(w).unproved?.length ?? 0) + (get(w).driven_only?.length ?? 0) : "");
	});
	delegated("click", button, () => set(view, "activity"));
	delegated("click", button_1, () => set(view, "sight"));
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/shell/tabs.svelte.ts
var KEY = "vp-tabs";
function load() {
	try {
		const v = JSON.parse(sessionStorage.getItem(KEY) ?? "[]");
		return Array.isArray(v) ? v.filter((t) => typeof t?.surface === "string") : [];
	} catch {
		return [];
	}
}
var tabs = proxy({
	list: load(),
	active: -1
});
function keep() {
	try {
		sessionStorage.setItem(KEY, JSON.stringify(tabs.list));
	} catch {}
}
var same = (t, surface, focus) => t.surface === surface && t.focus === focus;
function show(surface, focus, pin = false) {
	const at = tabs.list.findIndex((t) => same(t, surface, focus));
	if (at !== -1) {
		if (pin) tabs.list[at].pinned = true;
		tabs.active = at;
		keep();
		return;
	}
	const preview = tabs.list.findIndex((t) => !t.pinned);
	const tab = {
		surface,
		focus,
		pinned: pin
	};
	if (preview !== -1) {
		tabs.list[preview] = tab;
		tabs.active = preview;
	} else {
		tabs.list.push(tab);
		tabs.active = tabs.list.length - 1;
	}
	keep();
}
function pin() {
	const t = tabs.list[tabs.active];
	if (t && !t.pinned) {
		t.pinned = true;
		keep();
	}
}
function close(i) {
	if (i < 0 || i >= tabs.list.length) return null;
	tabs.list.splice(i, 1);
	if (tabs.list.length === 0) {
		tabs.active = -1;
		keep();
		return null;
	}
	if (tabs.active >= i) tabs.active = Math.max(0, tabs.active - 1);
	keep();
	return tabs.list[tabs.active];
}
function hashOf(t) {
	return `#${t.surface}${t.focus ? `/${encodeURIComponent(t.focus)}` : ""}`;
}
//#endregion
//#region src/shell/keys.ts
bind({
	surface: "global",
	combo: "Mod+b",
	action: "toggle-side",
	label: "show or hide the sidebar"
});
bind({
	surface: "global",
	combo: "Mod+j",
	action: "toggle-panel",
	label: "show or hide the activity panel"
});
bind({
	surface: "global",
	combo: "Alt+w",
	action: "close-tab",
	label: "close the tab"
});
bind({
	surface: "global",
	combo: "Alt+]",
	action: "next-tab",
	label: "next tab"
});
bind({
	surface: "global",
	combo: "Alt+[",
	action: "prev-tab",
	label: "previous tab"
});
bind({
	surface: "global",
	combo: "Alt+n",
	action: "new-change",
	label: "start a new change"
});
//#endregion
//#region src/App.svelte
var root = /* @__PURE__ */ from_html(`<p class="problem svelte-1n46o8q" role="alert"> </p>`);
var root_1 = /* @__PURE__ */ from_html(`<main id="surface" class="bare svelte-1n46o8q"><!> <!></main>`);
var root_2 = /* @__PURE__ */ from_html(`<aside class="side svelte-1n46o8q"><!></aside>`);
var root_3 = /* @__PURE__ */ from_html(`<p class="notice svelte-1n46o8q" role="status"> </p>`);
var root_4 = /* @__PURE__ */ from_html(`<p>No surface is registered.</p>`);
var root_5 = /* @__PURE__ */ from_html(`<!> <main id="surface" class="editor svelte-1n46o8q" tabindex="-1"><!> <!> <!></main>`, 1);
var root_6 = /* @__PURE__ */ from_html(`<div class="scrim svelte-1n46o8q" role="presentation"></div> <div class="overlay svelte-1n46o8q" role="dialog" aria-modal="true"><!></div>`, 1);
var root_7 = /* @__PURE__ */ from_html(`<span class="dim svelte-1n46o8q">· everywhere</span>`);
var root_8 = /* @__PURE__ */ from_html(`<dt><kbd class="svelte-1n46o8q"> </kbd></dt> <dd class="svelte-1n46o8q"> <!></dd>`, 1);
var root_9 = /* @__PURE__ */ from_html(`<div class="scrim svelte-1n46o8q" role="presentation"></div> <div class="help svelte-1n46o8q" role="dialog" aria-modal="true" aria-label="keys bound here"><header class="svelte-1n46o8q"><!> <h2 class="svelte-1n46o8q">Keys bound here</h2></header> <dl class="svelte-1n46o8q"></dl></div>`, 1);
var root_10 = /* @__PURE__ */ from_html(`<button class="skip svelte-1n46o8q">Skip to content</button> <div class="wb svelte-1n46o8q"><!> <div class="mid svelte-1n46o8q"><!> <!></div> <!></div> <!> <!>`, 1);
function App($$anchor, $$props) {
	push($$props, true);
	let current = /* @__PURE__ */ state(proxy(landing()?.id ?? ""));
	let focus = /* @__PURE__ */ state("");
	let notice = /* @__PURE__ */ state("");
	let helpOpen = /* @__PURE__ */ state(false);
	const showing = /* @__PURE__ */ user_derived(() => surfaces().find((s) => s.id === get(current)));
	const looking = () => get(showing)?.marksLook === true && document.visibilityState === "visible";
	const { state: feed, start, look } = live(looking);
	user_effect(() => untrack(start));
	user_effect(() => {
		if (get(showing)?.marksLook) untrack(look);
	});
	const { state: themeState, restore, cycle } = theme();
	user_effect(restore);
	function flag(key, dflt) {
		try {
			const v = localStorage.getItem(key);
			return v === null ? dflt : v === "1";
		} catch {
			return dflt;
		}
	}
	function keepFlag(key, v) {
		try {
			localStorage.setItem(key, v ? "1" : "0");
		} catch {}
	}
	let sideOpen = /* @__PURE__ */ state(proxy(flag("vp-side", true)));
	let panelOpen = /* @__PURE__ */ state(proxy(flag("vp-panel", false)));
	user_effect(() => keepFlag("vp-side", get(sideOpen)));
	user_effect(() => keepFlag("vp-panel", get(panelOpen)));
	function read() {
		const raw = location.hash.slice(1);
		const cut = raw.indexOf("/");
		const id = cut === -1 ? raw : raw.slice(0, cut);
		if (!surfaces().some((s) => s.id === id)) {
			if (!raw) {
				const home = landing()?.id;
				if (home) go(`#${home}`);
			}
			return;
		}
		set(current, id, true);
		set(focus, cut === -1 ? "" : decodeURIComponent(raw.slice(cut + 1)), true);
		set(helpOpen, false);
		const s = surfaces().find((x) => x.id === id);
		if (s && !s.transient && !s.bare) show(id, get(focus));
	}
	function readQuery() {
		claimToken();
		const url = new URL(location.href);
		const q = url.searchParams;
		if ([...q.keys()].length === 0) return;
		let id = "";
		let f = "";
		const want = q.get("surface");
		if (want && surfaces().some((s) => s.id === want)) id = want;
		for (const s of surfaces()) {
			const v = s.link ? q.get(s.link) : null;
			if (s.link && v) {
				id = s.id;
				f = s.linkFocus ? s.linkFocus(v) : v;
			}
		}
		const link = q.get("link");
		if (link) {
			set(notice, `${link} names nothing this app opens; links are change, review, ask, run, inbox`);
			id = landing()?.id ?? "";
			f = "";
		}
		const hash = id ? `#${id}${f ? `/${encodeURIComponent(f)}` : ""}` : url.hash;
		history.replaceState({}, "", `${url.pathname}${hash}`);
	}
	user_effect(() => {
		readQuery();
		read();
		addEventListener("hashchange", read);
		return () => removeEventListener("hashchange", read);
	});
	function pick(id) {
		const last = [...tabs.list].reverse().find((t) => t.surface === id);
		if (id === get(current) && get(sideOpen) && surfaces().find((s) => s.id === id)?.side) {
			set(sideOpen, false);
			return;
		}
		set(sideOpen, true);
		go(last ? hashOf(last) : `#${id}`);
	}
	function pickTab(i) {
		const t = tabs.list[i];
		if (t) go(hashOf(t));
	}
	function closeTab(i) {
		const next = close(i);
		go(next ? hashOf(next) : `#${landing()?.id ?? ""}`);
	}
	user_effect(() => {
		const onKey = (e) => {
			dispatch(get(current), e);
		};
		addEventListener("keydown", onKey);
		const offs = [
			onAction("help", () => {
				set(helpOpen, !get(helpOpen));
				return true;
			}),
			onAction("toggle-side", () => {
				set(sideOpen, !get(sideOpen));
				return true;
			}),
			onAction("toggle-panel", () => {
				set(panelOpen, !get(panelOpen));
				return true;
			}),
			onAction("close-tab", () => {
				if (tabs.active !== -1) closeTab(tabs.active);
				return true;
			}),
			onAction("next-tab", () => {
				if (tabs.list.length) pickTab((tabs.active + 1) % tabs.list.length);
				return true;
			}),
			onAction("prev-tab", () => {
				if (tabs.list.length) pickTab((tabs.active - 1 + tabs.list.length) % tabs.list.length);
				return true;
			}),
			onAction("leave", () => {
				if (get(helpOpen)) {
					set(helpOpen, false);
					return true;
				}
				if (get(notice)) {
					set(notice, "");
					return true;
				}
				return false;
			})
		];
		return () => {
			removeEventListener("keydown", onKey);
			offs.forEach((off) => off());
		};
	});
	const tabTitle = (t) => {
		const s = surfaces().find((x) => x.id === t.surface);
		if (!s) return t.surface;
		return t.focus && s.tab?.(feed, t.focus) || (t.focus ? `${s.title}: ${t.focus}` : s.title);
	};
	const tabIcon = (t) => surfaces().find((x) => x.id === t.surface)?.icon;
	const crumbs = /* @__PURE__ */ user_derived(() => get(showing) ? get(focus) && tabs.list[tabs.active] ? [get(showing).title, tabTitle(tabs.list[tabs.active])] : [get(showing).title] : []);
	const pulse = /* @__PURE__ */ user_derived(() => feed.unauthorised ? "no token" : feed.error ? "not answering" : feed.loaded ? "live" : "connecting");
	const projects = /* @__PURE__ */ user_derived(() => feed.board?.summary?.projects ?? null);
	const status = /* @__PURE__ */ user_derived(() => surfaces().flatMap((s) => (s.status?.(feed) ?? []).map((i) => ({
		...i,
		surface: s.id,
		title: s.title
	}))));
	const runsAt = /* @__PURE__ */ user_derived(() => surfaces().find((s) => s.holds === "run")?.id ?? "");
	const keys = /* @__PURE__ */ user_derived(() => help(get(current)));
	const under = /* @__PURE__ */ user_derived(() => {
		if (!get(showing)?.transient) return {
			s: get(showing),
			f: get(focus)
		};
		const t = tabs.list[tabs.active];
		return {
			s: (t ? surfaces().find((x) => x.id === t.surface) : void 0) ?? landing(),
			f: t?.focus ?? ""
		};
	});
	const page = /* @__PURE__ */ user_derived(() => get(under).s);
	const Side = /* @__PURE__ */ user_derived(() => get(page)?.side);
	function settler() {
		let last = {};
		return (next) => {
			const keys = Object.keys(next);
			let changed = keys.length !== Object.keys(last).length;
			const out = {};
			for (const k of keys) if (k in last && same$1(last[k], next[k])) out[k] = last[k];
			else {
				out[k] = next[k];
				changed = true;
			}
			if (changed) last = out;
			return last;
		};
	}
	const settlePage = settler();
	const settleOverlay = settler();
	const props = /* @__PURE__ */ user_derived(() => settlePage(get(page) ? get(page).select(feed, get(under).f) : {}));
	const overlayProps = /* @__PURE__ */ user_derived(() => settleOverlay(get(showing)?.transient ? get(showing).select(feed, get(focus)) : {}));
	function openFocus(f, pinIt = false) {
		if (!get(page)) return;
		go(`#${get(page).id}${f ? `/${encodeURIComponent(f)}` : ""}`);
		if (pinIt) pin();
	}
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_1 = ($$anchor) => {
		var main = root_1();
		var node_1 = child(main);
		var consequent = ($$anchor) => {
			var p = root();
			var text = only_child(p, true);
			template_effect(() => set_text(text, feed.error));
			append($$anchor, p);
		};
		if_block(node_1, ($$render) => {
			if (feed.unauthorised) $$render(consequent);
		});
		component(sibling(node_1, 2), () => get(showing).component, ($$anchor, showing_component) => {
			showing_component($$anchor, spread_props(() => get(props)));
		});
		reset(main);
		append($$anchor, main);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_1 = root_10();
		var button = first_child(fragment_1);
		var div = sibling(button, 2);
		var node_3 = child(div);
		TitleBar(node_3, {
			get crumbs() {
				return get(crumbs);
			},
			get sideOpen() {
				return get(sideOpen);
			},
			get panelOpen() {
				return get(panelOpen);
			},
			find: () => run("open-palette", get(current)),
			create: () => run("new-change", get(current)),
			toggleSide: () => set(sideOpen, !get(sideOpen)),
			togglePanel: () => set(panelOpen, !get(panelOpen))
		});
		var div_1 = sibling(node_3, 2);
		var node_4 = child(div_1);
		{
			let $0 = /* @__PURE__ */ user_derived(listed$1);
			ActivityBar(node_4, {
				get items() {
					return get($0);
				},
				get feed() {
					return feed;
				},
				get current() {
					return get(current);
				},
				pick
			});
		}
		var node_5 = sibling(node_4, 2);
		{
			const pane = ($$anchor) => {
				var aside = root_2();
				var node_6 = child(aside);
				var consequent_2 = ($$anchor) => {
					var fragment_2 = comment();
					component(first_child(fragment_2), () => get(Side), ($$anchor, Side_1) => {
						Side_1($$anchor, spread_props(() => get(props), {
							get focus() {
								return get(focus);
							},
							open: openFocus
						}));
					});
					append($$anchor, fragment_2);
				};
				if_block(node_6, ($$render) => {
					if (get(Side)) $$render(consequent_2);
				});
				reset(aside);
				template_effect(() => set_attribute(aside, "aria-label", `${get(showing)?.title ?? "" ?? ""} list`));
				append($$anchor, aside);
			};
			let $0 = /* @__PURE__ */ user_derived(() => !get(sideOpen) || !get(Side));
			Split(node_5, {
				id: "side",
				size: 300,
				min: 200,
				max: 560,
				get collapsed() {
					return get($0);
				},
				pane,
				children: ($$anchor, $$slotProps) => {
					{
						const pane = ($$anchor) => {
							{
								let $0 = /* @__PURE__ */ user_derived(() => get(runsAt) ? (id) => go(`#${get(runsAt)}/${encodeURIComponent(id)}`) : null);
								Panel($$anchor, {
									get board() {
										return feed.board;
									},
									get error() {
										return feed.error;
									},
									get open() {
										return get($0);
									}
								});
							}
						};
						let $0 = /* @__PURE__ */ user_derived(() => !get(panelOpen));
						Split($$anchor, {
							id: "panel",
							axis: "y",
							side: "end",
							size: 220,
							min: 120,
							max: 560,
							get collapsed() {
								return get($0);
							},
							pane,
							children: ($$anchor, $$slotProps) => {
								var fragment_5 = root_5();
								var node_8 = first_child(fragment_5);
								var consequent_3 = ($$anchor) => {
									EditorTabs($$anchor, {
										get list() {
											return tabs.list;
										},
										get active() {
											return tabs.active;
										},
										title: tabTitle,
										icon: tabIcon,
										pick: pickTab,
										close: closeTab,
										pin: (i) => {
											pickTab(i);
											pin();
										}
									});
								};
								if_block(node_8, ($$render) => {
									if (tabs.list.length > 0) $$render(consequent_3);
								});
								var main_1 = sibling(node_8, 2);
								var node_9 = child(main_1);
								var consequent_4 = ($$anchor) => {
									var p_1 = root();
									var text_1 = only_child(p_1, true);
									template_effect(() => set_text(text_1, feed.error));
									append($$anchor, p_1);
								};
								var consequent_5 = ($$anchor) => {
									Stale($$anchor, {
										get error() {
											return feed.error;
										},
										get stale_since() {
											return feed.stale_since;
										}
									});
								};
								if_block(node_9, ($$render) => {
									if (feed.unauthorised) $$render(consequent_4);
									else if (feed.error) $$render(consequent_5, 1);
								});
								var node_10 = sibling(node_9, 2);
								var consequent_6 = ($$anchor) => {
									var p_2 = root_3();
									var text_2 = only_child(p_2, true);
									template_effect(() => set_text(text_2, get(notice)));
									append($$anchor, p_2);
								};
								if_block(node_10, ($$render) => {
									if (get(notice)) $$render(consequent_6);
								});
								var node_11 = sibling(node_10, 2);
								var consequent_7 = ($$anchor) => {
									var fragment_8 = comment();
									component(first_child(fragment_8), () => get(page).component, ($$anchor, page_component) => {
										page_component($$anchor, spread_props(() => get(props), {
											get focus() {
												return get(under).f;
											},
											open: openFocus
										}));
									});
									append($$anchor, fragment_8);
								};
								var alternate = ($$anchor) => {
									append($$anchor, root_4());
								};
								if_block(node_11, ($$render) => {
									if (get(page)) $$render(consequent_7);
									else $$render(alternate, -1);
								});
								reset(main_1);
								template_effect(() => set_attribute(main_1, "aria-label", get(page)?.heading));
								append($$anchor, fragment_5);
							},
							$$slots: {
								pane: true,
								default: true
							}
						});
					}
				},
				$$slots: {
					pane: true,
					default: true
				}
			});
		}
		reset(div_1);
		var node_13 = sibling(div_1, 2);
		{
			let $0 = /* @__PURE__ */ user_derived(() => !!feed.error || feed.unauthorised);
			let $1 = /* @__PURE__ */ user_derived(() => themeState.choice === "system" ? "auto" : themeState.choice);
			StatusBar(node_13, {
				get items() {
					return get(status);
				},
				get projects() {
					return get(projects);
				},
				get pulse() {
					return get(pulse);
				},
				get bad() {
					return get($0);
				},
				get theme() {
					return get($1);
				},
				get cycleTheme() {
					return cycle;
				},
				help: () => set(helpOpen, !get(helpOpen)),
				go: (h) => go(h)
			});
		}
		reset(div);
		var node_14 = sibling(div, 2);
		var consequent_8 = ($$anchor) => {
			var fragment_9 = root_6();
			var div_2 = first_child(fragment_9);
			var div_3 = sibling(div_2, 2);
			component(child(div_3), () => get(showing).component, ($$anchor, showing_component_1) => {
				showing_component_1($$anchor, spread_props(() => get(overlayProps)));
			});
			reset(div_3);
			action(div_3, ($$node) => trap?.($$node));
			template_effect(() => set_attribute(div_3, "aria-label", get(showing).title));
			delegated("click", div_2, () => run("leave", get(current)));
			append($$anchor, fragment_9);
		};
		if_block(node_14, ($$render) => {
			if (get(showing)?.transient) $$render(consequent_8);
		});
		var node_16 = sibling(node_14, 2);
		var consequent_10 = ($$anchor) => {
			var fragment_10 = root_9();
			var div_4 = first_child(fragment_10);
			var div_5 = sibling(div_4, 2);
			var header = child(div_5);
			Icon(child(header), {
				name: "keyboard",
				size: 16
			});
			next(2);
			reset(header);
			var dl = sibling(header, 2);
			each(dl, 21, () => get(keys), (k) => k.surface + k.combo, ($$anchor, k) => {
				var fragment_11 = root_8();
				var dt = first_child(fragment_11);
				var text_3 = only_child(child(dt), true);
				reset(dt);
				var dd = sibling(dt, 2);
				var text_4 = child(dd, true);
				var node_18 = sibling(text_4);
				var consequent_9 = ($$anchor) => {
					append($$anchor, root_7());
				};
				if_block(node_18, ($$render) => {
					if (get(k).surface === "global") $$render(consequent_9);
				});
				reset(dd);
				template_effect(($0) => {
					set_text(text_3, $0);
					set_text(text_4, get(k).label);
				}, [() => spell(get(k).combo)]);
				append($$anchor, fragment_11);
			});
			reset(dl);
			reset(div_5);
			action(div_5, ($$node) => trap?.($$node));
			delegated("click", div_4, () => set(helpOpen, false));
			append($$anchor, fragment_10);
		};
		if_block(node_16, ($$render) => {
			if (get(helpOpen)) $$render(consequent_10);
		});
		delegated("click", button, () => document.getElementById("surface")?.focus());
		append($$anchor, fragment_1);
	};
	if_block(node, ($$render) => {
		if (get(showing)?.bare) $$render(consequent_1);
		else $$render(alternate_1, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/main.ts
var target = document.getElementById("app");
if (!target) throw new Error("no #app to mount into");
mount(App, { target });
//#endregion

//# sourceMappingURL=app.js.map