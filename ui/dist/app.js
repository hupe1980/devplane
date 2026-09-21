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
var listeners = /* @__PURE__ */ new Map();
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
					var counts = listeners.get(node);
					if (counts === void 0) {
						counts = /* @__PURE__ */ new Map();
						listeners.set(node, counts);
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
				var counts = listeners.get(node);
				var count = counts.get(event_name);
				if (--count == 0) {
					node.removeEventListener(event_name, handle_event_propagation);
					counts.delete(event_name);
					if (counts.size === 0) listeners.delete(node);
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
/** @type {Set<HTMLInputElement[]>} */
var pending = /* @__PURE__ */ new Set();
/**
* @param {HTMLInputElement[]} inputs
* @param {null | [number]} group_index
* @param {HTMLInputElement} input
* @param {() => unknown} get
* @param {(value: unknown) => void} set
* @returns {void}
*/
function bind_group(inputs, group_index, input, get, set = get) {
	var is_checkbox = input.getAttribute("type") === "checkbox";
	var binding_group = inputs;
	let hydration_mismatch = false;
	if (group_index !== null) for (var index of group_index) binding_group = binding_group[index] ??= [];
	binding_group.push(input);
	listen_to_event_and_reset_event(input, "change", () => {
		var value = input.__value;
		if (is_checkbox) value = get_binding_group_value(binding_group, value, input.checked);
		set(value);
	}, () => set(is_checkbox ? [] : null));
	render_effect(() => {
		var value = get();
		if (hydrating && input.defaultChecked !== input.checked) {
			hydration_mismatch = true;
			return;
		}
		if (is_checkbox) {
			value = value || [];
			input.checked = value.includes(input.__value);
		} else input.checked = is(input.__value, value);
	});
	teardown(() => {
		var index = binding_group.indexOf(input);
		if (index !== -1) binding_group.splice(index, 1);
	});
	if (!pending.has(binding_group)) {
		pending.add(binding_group);
		queue_micro_task(() => {
			binding_group.sort((a, b) => a.compareDocumentPosition(b) === 4 ? -1 : 1);
			pending.delete(binding_group);
		});
	}
	queue_micro_task(() => {
		if (hydration_mismatch) {
			var value;
			if (is_checkbox) value = get_binding_group_value(binding_group, value, input.checked);
			else value = binding_group.find((input) => input.checked)?.__value;
			set(value);
		}
	});
}
/**
* @template V
* @param {Array<HTMLInputElement>} group
* @param {V} __value
* @param {boolean} checked
* @returns {V[]}
*/
function get_binding_group_value(group, __value, checked) {
	/** @type {Set<V>} */
	var value = /* @__PURE__ */ new Set();
	for (var i = 0; i < group.length; i += 1) if (group[i].checked) value.add(group[i].__value);
	if (!checked) value.delete(__value);
	return Array.from(value);
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
var BANDS = [
	"attention",
	"doing",
	"project"
];
var registry = /* @__PURE__ */ new Map();
function register(surface) {
	if (registry.has(surface.id)) throw new Error(`two surfaces claim the id "${surface.id}"`);
	registry.set(surface.id, surface);
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
function ago(seconds) {
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
//#endregion
//#region src/lib/api.ts
var KEY$1 = "vp_token";
function claimToken() {
	try {
		const url = new URL(location.href);
		const fromUrl = url.searchParams.get("token");
		if (fromUrl) {
			sessionStorage.setItem(KEY$1, fromUrl);
			url.searchParams.delete("token");
			history.replaceState({}, "", url);
			return fromUrl;
		}
		return sessionStorage.getItem(KEY$1) ?? "";
	} catch {
		return "";
	}
}
var Unauthorised = class extends Error {
	constructor() {
		super("unauthorised — run `devplane open` again");
	}
};
async function api(path, opts = {}) {
	const token = claimToken();
	const res = await fetch(path, {
		...opts,
		headers: {
			...opts.headers ?? {},
			Authorization: `Bearer ${token}`
		}
	});
	if (res.status === 401) throw new Unauthorised();
	if (!res.ok) throw new Error(`${path} → ${res.status}`);
	return await res.json();
}
//#endregion
//#region src/surfaces/board/Counts.svelte
var root$12 = /* @__PURE__ */ from_html(`<span class="card loud wait svelte-ehfks7"><b class="svelte-ehfks7"> </b> need you</span>`);
var root_1$12 = /* @__PURE__ */ from_html(`<span class="card loud wait svelte-ehfks7"><b class="svelte-ehfks7"> </b> waiting on you</span>`);
var root_2$11 = /* @__PURE__ */ from_html(`<span class="card loud fail svelte-ehfks7"><b class="svelte-ehfks7"> </b> failed</span>`);
var root_3$11 = /* @__PURE__ */ from_html(`<span class="card svelte-ehfks7"><b class="svelte-ehfks7"> </b> idle</span>`);
var root_4$10 = /* @__PURE__ */ from_html(`<span class="card svelte-ehfks7"><b class="svelte-ehfks7"> </b> spent</span>`);
var root_5$8 = /* @__PURE__ */ from_html(`<span class="wait svelte-ehfks7"> </span>`);
var root_6$6 = /* @__PURE__ */ from_html(`<span class="card svelte-ehfks7"><b class="svelte-ehfks7"> </b> issues &amp; PRs<!></span>`);
var root_7$5 = /* @__PURE__ */ from_html(`<span class="card quiet svelte-ehfks7"> </span>`);
var root_8$5 = /* @__PURE__ */ from_html(`<div class="counts svelte-ehfks7"><!> <!> <!> <span class="card svelte-ehfks7"><b class="svelte-ehfks7"> </b> working</span> <!> <span class="card svelte-ehfks7"><b class="svelte-ehfks7"> </b> sessions</span> <span class="card svelte-ehfks7"><b class="svelte-ehfks7"> </b> projects</span> <!> <!> <!></div>`);
function Counts($$anchor, $$props) {
	push($$props, true);
	var div = root_8$5();
	var node = child(div);
	var consequent = ($$anchor) => {
		var span = root$12();
		var text = only_child(child(span), true);
		next();
		reset(span);
		template_effect(() => set_text(text, $$props.summary.needs_you));
		append($$anchor, span);
	};
	if_block(node, ($$render) => {
		if ($$props.summary.needs_you) $$render(consequent);
	});
	var node_1 = sibling(node, 2);
	var consequent_1 = ($$anchor) => {
		var span_1 = root_1$12();
		var text_1 = only_child(child(span_1), true);
		next();
		reset(span_1);
		template_effect(() => set_text(text_1, $$props.summary.asks_waiting));
		append($$anchor, span_1);
	};
	if_block(node_1, ($$render) => {
		if ($$props.summary.asks_waiting) $$render(consequent_1);
	});
	var node_2 = sibling(node_1, 2);
	var consequent_2 = ($$anchor) => {
		var span_2 = root_2$11();
		var text_2 = only_child(child(span_2), true);
		next();
		reset(span_2);
		template_effect(() => set_text(text_2, $$props.summary.failed));
		append($$anchor, span_2);
	};
	if_block(node_2, ($$render) => {
		if ($$props.summary.failed) $$render(consequent_2);
	});
	var span_3 = sibling(node_2, 2);
	var text_3 = only_child(child(span_3), true);
	next();
	reset(span_3);
	var node_3 = sibling(span_3, 2);
	var consequent_3 = ($$anchor) => {
		var span_4 = root_3$11();
		var text_4 = only_child(child(span_4), true);
		next();
		reset(span_4);
		template_effect(() => set_text(text_4, $$props.summary.idle));
		append($$anchor, span_4);
	};
	if_block(node_3, ($$render) => {
		if ($$props.summary.idle) $$render(consequent_3);
	});
	var span_5 = sibling(node_3, 2);
	var text_5 = only_child(child(span_5), true);
	next();
	reset(span_5);
	var span_6 = sibling(span_5, 2);
	var text_6 = only_child(child(span_6), true);
	next();
	reset(span_6);
	var node_4 = sibling(span_6, 2);
	var consequent_4 = ($$anchor) => {
		var span_7 = root_4$10();
		var text_7 = only_child(child(span_7));
		next();
		reset(span_7);
		template_effect(($0) => set_text(text_7, `$${$0 ?? ""}`), [() => $$props.summary.cost_usd.toFixed(2)]);
		append($$anchor, span_7);
	};
	if_block(node_4, ($$render) => {
		if ($$props.summary.cost_usd > 0) $$render(consequent_4);
	});
	var node_5 = sibling(node_4, 2);
	var consequent_6 = ($$anchor) => {
		var span_8 = root_6$6();
		var b_8 = child(span_8);
		var text_8 = only_child(b_8, true);
		var node_6 = sibling(b_8, 2);
		var consequent_5 = ($$anchor) => {
			var span_9 = root_5$8();
			var text_9 = only_child(span_9);
			template_effect(() => set_text(text_9, `· ${$$props.summary.forge_needs_you ?? ""} need you`));
			append($$anchor, span_9);
		};
		if_block(node_6, ($$render) => {
			if ($$props.summary.forge_needs_you) $$render(consequent_5);
		});
		reset(span_8);
		template_effect(() => set_text(text_8, $$props.summary.open_issues + $$props.summary.open_prs));
		append($$anchor, span_8);
	};
	if_block(node_5, ($$render) => {
		if ($$props.summary.open_issues + $$props.summary.open_prs > 0) $$render(consequent_6);
	});
	var node_7 = sibling(node_5, 2);
	var consequent_7 = ($$anchor) => {
		var span_10 = root_7$5();
		var text_10 = only_child(span_10);
		template_effect(() => set_text(text_10, `${$$props.summary.dormant ?? ""} quiet`));
		append($$anchor, span_10);
	};
	if_block(node_7, ($$render) => {
		if ($$props.summary.dormant) $$render(consequent_7);
	});
	reset(div);
	template_effect(() => {
		set_text(text_3, $$props.summary.working);
		set_text(text_5, $$props.summary.runs);
		set_text(text_6, $$props.summary.projects);
	});
	append($$anchor, div);
	pop();
}
//#endregion
//#region src/surfaces/board/Supervision.svelte
var root$11 = /* @__PURE__ */ from_html(`<span class="pm nobody svelte-1ewfh4c" title="This session decides without you. Devplane reads the mode and never sets it."> </span>`);
var root_1$11 = /* @__PURE__ */ from_html(`<span class="pm unsure svelte-1ewfh4c" title="A permission mode this build does not recognise, so whether anybody is asked cannot be said."> </span>`);
function Supervision($$anchor, $$props) {
	let mode = prop($$props, "mode", 3, null), asksAPerson = prop($$props, "asksAPerson", 3, null);
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var span = root$11();
		var text = only_child(span, true);
		template_effect(() => set_text(text, mode()));
		append($$anchor, span);
	};
	var consequent_1 = ($$anchor) => {
		var span_1 = root_1$11();
		var text_1 = only_child(span_1);
		template_effect(() => set_text(text_1, `${mode() ?? ""} ?`));
		append($$anchor, span_1);
	};
	if_block(node, ($$render) => {
		if (mode() !== null && asksAPerson() === false) $$render(consequent);
		else if (mode() !== null && asksAPerson() === null) $$render(consequent_1, 1);
	});
	append($$anchor, fragment);
}
//#endregion
//#region src/surfaces/board/Board.svelte
var root$10 = /* @__PURE__ */ from_html(`<span class="miss svelte-13ck13b"><b> </b> </span>`);
var root_1$10 = /* @__PURE__ */ from_html(`<p class="coverage svelte-13ck13b" role="status"> <!></p>`);
var root_2$10 = /* @__PURE__ */ from_html(`<p class="seen svelte-13ck13b"> </p>`);
var root_3$10 = /* @__PURE__ */ from_html(`<p class="unseen svelte-13ck13b"> </p>`);
var root_4$9 = /* @__PURE__ */ from_html(`<p class="svelte-13ck13b"><b>Nothing is running that Devplane can see.</b></p> <p class="seen svelte-13ck13b"> </p> <!> <!>`, 1);
var root_5$7 = /* @__PURE__ */ from_html(`<p class="svelte-13ck13b"><b>Nothing is running that Devplane can see.</b></p>`);
var root_6$5 = /* @__PURE__ */ from_html(`<div class="empty svelte-13ck13b"><!></div>`);
var root_7$4 = /* @__PURE__ */ from_html(`<span class="sr svelte-13ck13b">context nearly full</span>`);
var root_8$4 = /* @__PURE__ */ from_html(`<li role="listitem" class="svelte-13ck13b"><span><span class="dot svelte-13ck13b" aria-hidden="true"></span> </span> <span class="what svelte-13ck13b"><a class="says svelte-13ck13b"> <span class="sr svelte-13ck13b">— why this is here</span></a> <span class="sub svelte-13ck13b"><span class="who svelte-13ck13b"> </span> <span> </span> <!></span></span> <span class="nums svelte-13ck13b"><span class="cost svelte-13ck13b"> </span> <span> <!></span> <span class="idle svelte-13ck13b"> </span></span> <span class="acts svelte-13ck13b"><button title="raise the window that owns this run" class="svelte-13ck13b">focus</button> <button title="copy \`devplane attach\`" class="svelte-13ck13b">attach</button></span> <span class="sr svelte-13ck13b"> </span></li>`);
var root_9$4 = /* @__PURE__ */ from_html(`<h3 class="svelte-13ck13b"> <span class="n svelte-13ck13b"> </span></h3> <ul role="list" class="svelte-13ck13b"></ul>`, 1);
var root_10$3 = /* @__PURE__ */ from_html(`<section aria-labelledby="board-head"><h2 id="board-head" class="svelte-13ck13b">What is happening</h2> <!> <!> <p class="said svelte-13ck13b" role="status" aria-live="polite"> </p> <!></section>`);
function Board($$anchor, $$props) {
	push($$props, true);
	const NOTHING = {
		projects: 0,
		runs: 0,
		working: 0,
		needs_you: 0,
		idle: 0,
		failed: 0,
		dormant: 0,
		cost_usd: 0,
		open_issues: 0,
		open_prs: 0,
		forge_needs_you: 0,
		asks_waiting: 0
	};
	let said = /* @__PURE__ */ state("");
	async function focus(run) {
		try {
			await api(`/api/runs/${encodeURIComponent(run.id)}/focus`, { method: "POST" });
			set(said, "raised the window that owns it");
		} catch (e) {
			set(said, `that did not land: ${e instanceof Error ? e.message : String(e)}`);
		}
	}
	async function attach(run) {
		const cmd = `devplane attach ${run.id}`;
		try {
			await navigator.clipboard?.writeText(cmd);
			set(said, `copied: ${cmd}`);
		} catch {
			set(said, `no clipboard here — run: ${cmd}`);
		}
	}
	let runs = prop($$props, "runs", 19, () => []), summary = prop($$props, "summary", 3, NOTHING), coverage = prop($$props, "coverage", 3, null), thresholds = prop($$props, "thresholds", 3, null), watching = prop($$props, "watching", 3, null);
	function listed(xs) {
		if (xs.length <= 1) return xs[0] ?? "";
		return `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`;
	}
	function crowded(pct) {
		const at = thresholds()?.context_high_percent;
		return pct !== null && typeof at === "number" && pct >= at;
	}
	const grouped = /* @__PURE__ */ user_derived(() => Object.entries(runs().reduce((acc, r) => {
		const k = r.project_name ?? "(no project)";
		(acc[k] ??= []).push(r);
		return acc;
	}, {})).sort(([a], [b]) => a.localeCompare(b)));
	var section = root_10$3();
	var node = sibling(child(section), 2);
	Counts(node, { get summary() {
		return summary();
	} });
	var node_1 = sibling(node, 2);
	var consequent = ($$anchor) => {
		var p = root_1$10();
		var text = child(p);
		each(sibling(text), 17, () => coverage().unreadable, (u) => u.name, ($$anchor, u) => {
			var span = root$10();
			var b_1 = child(span);
			var text_1 = only_child(b_1, true);
			var text_2 = sibling(b_1);
			reset(span);
			template_effect(() => {
				set_text(text_1, get(u).name);
				set_text(text_2, ` — ${get(u).why ?? ""}`);
			});
			append($$anchor, span);
		});
		reset(p);
		template_effect(() => set_text(text, `This is ${coverage().projects - coverage().unreadable.length} of ${coverage().projects ?? ""} projects. `));
		append($$anchor, p);
	};
	if_block(node_1, ($$render) => {
		if (coverage() && coverage().unreadable.length > 0) $$render(consequent);
	});
	var p_1 = sibling(node_1, 2);
	var text_3 = only_child(p_1, true);
	var node_3 = sibling(p_1, 2);
	var consequent_4 = ($$anchor) => {
		var div = root_6$5();
		var node_4 = child(div);
		var consequent_3 = ($$anchor) => {
			var fragment = root_4$9();
			var p_2 = sibling(first_child(fragment), 2);
			var text_4 = only_child(p_2);
			var node_5 = sibling(p_2, 2);
			var consequent_1 = ($$anchor) => {
				var p_3 = root_2$10();
				var text_5 = only_child(p_3);
				template_effect(($0) => set_text(text_5, `${$0 ?? ""}: the channels are read and that path has not been proved
            end to end yet.`), [() => listed(watching().unproved)]);
				append($$anchor, p_3);
			};
			if_block(node_5, ($$render) => {
				if (watching().unproved.length > 0) $$render(consequent_1);
			});
			var node_6 = sibling(node_5, 2);
			var consequent_2 = ($$anchor) => {
				var p_4 = root_3$10();
				var text_6 = only_child(p_4);
				template_effect(($0) => set_text(text_6, `${$0 ?? ""} appear only when Devplane starts them. A session you
            opened yourself in one of those is not on this board.`), [() => listed(watching().driven_only)]);
				append($$anchor, p_4);
			};
			if_block(node_6, ($$render) => {
				if (watching().driven_only.length > 0) $$render(consequent_2);
			});
			template_effect(($0) => set_text(text_4, `It watches ${$0 ?? ""} sessions you started yourself — start one and it
          appears here with no configuration.`), [() => listed(watching().watched)]);
			append($$anchor, fragment);
		};
		var alternate = ($$anchor) => {
			append($$anchor, root_5$7());
		};
		if_block(node_4, ($$render) => {
			if (watching() && watching().watched.length > 0) $$render(consequent_3);
			else $$render(alternate, -1);
		});
		reset(div);
		append($$anchor, div);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_1 = comment();
		each(first_child(fragment_1), 17, () => get(grouped), ([project, rows]) => project, ($$anchor, $$item) => {
			var $$array = /* @__PURE__ */ user_derived(() => to_array(get($$item), 2));
			let project = () => get($$array)[0];
			let rows = () => get($$array)[1];
			var fragment_2 = root_9$4();
			var h3 = first_child(fragment_2);
			var text_7 = child(h3);
			var text_8 = only_child(sibling(text_7), true);
			reset(h3);
			var ul = sibling(h3, 2);
			each(ul, 23, rows, (r) => r.id, ($$anchor, r, i) => {
				var li = root_8$4();
				var span_2 = child(li);
				var text_9 = sibling(child(span_2), 1, true);
				reset(span_2);
				var span_3 = sibling(span_2, 2);
				var a_1 = child(span_3);
				var text_10 = child(a_1, true);
				next();
				reset(a_1);
				var span_4 = sibling(a_1, 2);
				var span_5 = child(span_4);
				var text_11 = only_child(span_5, true);
				var span_6 = sibling(span_5, 2);
				var text_12 = only_child(span_6, true);
				var node_8 = sibling(span_6, 2);
				{
					let $0 = /* @__PURE__ */ user_derived(() => get(r).permission_mode ?? null);
					let $1 = /* @__PURE__ */ user_derived(() => get(r).asks_a_person ?? null);
					Supervision(node_8, {
						get mode() {
							return get($0);
						},
						get asksAPerson() {
							return get($1);
						}
					});
				}
				reset(span_4);
				reset(span_3);
				var span_7 = sibling(span_3, 2);
				var span_8 = child(span_7);
				var text_13 = only_child(span_8, true);
				var span_9 = sibling(span_8, 2);
				let classes;
				var text_14 = child(span_9);
				var node_9 = sibling(text_14);
				var consequent_5 = ($$anchor) => {
					append($$anchor, root_7$4());
				};
				var d = /* @__PURE__ */ user_derived(() => crowded(get(r).context_percent ?? null));
				if_block(node_9, ($$render) => {
					if (get(d)) $$render(consequent_5);
				});
				reset(span_9);
				var text_15 = only_child(sibling(span_9, 2), true);
				reset(span_7);
				var span_12 = sibling(span_7, 2);
				var button = child(span_12);
				var button_1 = sibling(button, 2);
				reset(span_12);
				var text_16 = only_child(sibling(span_12, 2));
				reset(li);
				template_effect(($0, $1, $2, $3, $4) => {
					set_class(span_2, 1, `state s-${get(r).state ?? ""}`, "svelte-13ck13b");
					set_text(text_9, get(r).state);
					set_attribute(a_1, "href", `#why/${$0 ?? ""}`);
					set_text(text_10, get(r).summary ?? "—");
					set_text(text_11, $1);
					set_text(text_12, get(r).agent);
					set_text(text_13, $2);
					classes = set_class(span_9, 1, "ctx svelte-13ck13b", null, classes, { crowded: $3 });
					set_text(text_14, `${get(r).context_percent === null ? "–" : `${get(r).context_percent}%`} `);
					set_text(text_15, $4);
					set_text(text_16, `row ${get(i) + 1} of ${rows().length ?? ""}`);
				}, [
					() => encodeURIComponent(get(r).id),
					() => clip(get(r).id, 8),
					() => get(r).cost_usd > 0 ? `$${get(r).cost_usd.toFixed(2)}` : "–",
					() => crowded(get(r).context_percent ?? null),
					() => ago(get(r).idle_seconds)
				]);
				delegated("click", button, () => focus(get(r)));
				delegated("click", button_1, () => attach(get(r)));
				append($$anchor, li);
			});
			reset(ul);
			template_effect(() => {
				set_text(text_7, `${project() ?? ""} `);
				set_text(text_8, rows().length);
			});
			append($$anchor, fragment_2);
		});
		append($$anchor, fragment_1);
	};
	if_block(node_3, ($$render) => {
		if (runs().length === 0) $$render(consequent_4);
		else $$render(alternate_1, -1);
	});
	reset(section);
	template_effect(() => set_text(text_3, get(said)));
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/board/index.ts
register({
	id: "board",
	title: "What is happening",
	band: "attention",
	order: 1,
	ports: ["attach", "focus"],
	select: (feed) => {
		const b = feed.board;
		return {
			runs: b?.runs ?? [],
			summary: b?.summary,
			coverage: b?.coverage ?? null,
			thresholds: b?.thresholds ?? null,
			watching: b?.watching ?? null
		};
	},
	component: Board
});
//#endregion
//#region src/surfaces/changes/Changes.svelte
var root$9 = /* @__PURE__ */ from_html(`<option> </option>`);
var root_1$9 = /* @__PURE__ */ from_html(`<p class="dim svelte-ap3eez">reading the checkout…</p>`);
var root_2$9 = /* @__PURE__ */ from_html(`<p class="dim svelte-ap3eez">Pick a Work to see the change it made against its base branch.</p>`);
var root_3$9 = /* @__PURE__ */ from_html(`<p class="finding svelte-ap3eez">This branch changed nothing against <code> </code>. Any check that passed here
        passed over no change.</p>`);
var root_4$8 = /* @__PURE__ */ from_html(`<p class="dim svelte-ap3eez"> </p>`);
var root_5$6 = /* @__PURE__ */ from_html(`<span> </span>
`, 1);
var root_6$4 = /* @__PURE__ */ from_html(`<pre class="hunk svelte-ap3eez"><code class="svelte-ap3eez"><span class="hdr svelte-ap3eez"> </span>
<!></code></pre>`);
var root_7$3 = /* @__PURE__ */ from_html(`<article class="file svelte-ap3eez"><h3 class="svelte-ap3eez"><span class="path svelte-ap3eez"> </span> <span class="status svelte-ap3eez"> </span> <span class="counts svelte-ap3eez"><b class="add svelte-ap3eez"> </b> <b class="del svelte-ap3eez"> </b></span></h3> <!></article>`);
var root_8$3 = /* @__PURE__ */ from_html(`<p class="finding svelte-ap3eez"> <code> </code></p>`);
var root_9$3 = /* @__PURE__ */ from_html(`<p class="base svelte-ap3eez">against <code> </code> · <b class="add svelte-ap3eez"> </b> <b class="del svelte-ap3eez"> </b> </p> <!> <!> <!>`, 1);
var root_10$2 = /* @__PURE__ */ from_html(`<section aria-labelledby="changes-head"><h2 id="changes-head" class="svelte-ap3eez">What changed</h2> <p class="pick svelte-ap3eez"><label for="which" class="svelte-ap3eez">Work</label> <select id="which"><option>choose one</option><!></select></p> <p class="said svelte-ap3eez" role="status" aria-live="polite"> </p> <!></section>`);
function Changes($$anchor, $$props) {
	push($$props, true);
	let works = prop($$props, "works", 19, () => []);
	let chosen = /* @__PURE__ */ state("");
	let set$1 = /* @__PURE__ */ state(null);
	let said = /* @__PURE__ */ state("");
	let loading = /* @__PURE__ */ state(false);
	async function load(id) {
		set(chosen, id, true);
		set(set$1, null);
		set(said, "");
		if (!id) return;
		set(loading, true);
		try {
			const body = await api(`/api/work/${encodeURIComponent(id)}/changes`);
			set(set$1, body.changes, true);
		} catch (e) {
			set(said, e instanceof Error ? e.message : String(e), true);
		} finally {
			set(loading, false);
		}
	}
	const totals = /* @__PURE__ */ user_derived(() => (get(set$1)?.files ?? []).reduce((acc, f) => ({
		added: acc.added + f.added,
		removed: acc.removed + f.removed
	}), {
		added: 0,
		removed: 0
	}));
	function statusWord(s) {
		return typeof s === "string" ? s : `renamed from ${s.renamed.from}`;
	}
	function hunks(b) {
		return "hunks" in b ? b.hunks : [];
	}
	var section = root_10$2();
	var p = sibling(child(section), 2);
	var select = sibling(child(p), 2);
	var option = child(select);
	option.value = option.__value = "";
	each(sibling(option), 17, works, (w) => w.id, ($$anchor, w) => {
		var option_1 = root$9();
		var text_1 = only_child(option_1, true);
		var option_1_value = {};
		template_effect(() => {
			set_text(text_1, get(w).title ?? get(w).id);
			if (option_1_value !== (option_1_value = get(w).id)) option_1.value = (option_1.__value = option_1_value) ?? "";
		});
		append($$anchor, option_1);
	});
	reset(select);
	var select_value;
	init_select(select);
	reset(p);
	var p_1 = sibling(p, 2);
	var text_2 = only_child(p_1, true);
	var node_1 = sibling(p_1, 2);
	var consequent = ($$anchor) => {
		append($$anchor, root_1$9());
	};
	var consequent_1 = ($$anchor) => {
		append($$anchor, root_2$9());
	};
	var consequent_6 = ($$anchor) => {
		var fragment = root_9$3();
		var p_4 = first_child(fragment);
		var code = sibling(child(p_4));
		var text_3 = only_child(code, true);
		var b_1 = sibling(code, 2);
		var text_4 = only_child(b_1);
		var b_2 = sibling(b_1, 2);
		var text_5 = only_child(b_2);
		var text_6 = sibling(b_2);
		reset(p_4);
		var node_2 = sibling(p_4, 2);
		var consequent_2 = ($$anchor) => {
			var p_5 = root_3$9();
			var text_7 = only_child(sibling(child(p_5)), true);
			next();
			reset(p_5);
			template_effect(() => set_text(text_7, get(set$1).base));
			append($$anchor, p_5);
		};
		if_block(node_2, ($$render) => {
			if (get(set$1).files.length === 0) $$render(consequent_2);
		});
		var node_3 = sibling(node_2, 2);
		each(node_3, 17, () => get(set$1).files, (f) => f.path, ($$anchor, f) => {
			var article = root_7$3();
			var h3 = child(article);
			var span = child(h3);
			var text_8 = only_child(span, true);
			var span_1 = sibling(span, 2);
			var text_9 = only_child(span_1, true);
			var span_2 = sibling(span_1, 2);
			var b_3 = child(span_2);
			var text_10 = only_child(b_3);
			var text_11 = only_child(sibling(b_3, 2));
			reset(span_2);
			reset(h3);
			var node_4 = sibling(h3, 2);
			var consequent_3 = ($$anchor) => {
				var p_6 = root_4$8();
				var text_12 = only_child(p_6);
				template_effect(() => set_text(text_12, `binary${get(f).body.binary.bytes === null ? "" : `, ${get(f).body.binary.bytes} bytes`}`));
				append($$anchor, p_6);
			};
			var consequent_4 = ($$anchor) => {
				var p_7 = root_4$8();
				var text_13 = only_child(p_7);
				template_effect(() => set_text(text_13, `not shown — ${get(f).body.skipped.why ?? ""}`));
				append($$anchor, p_7);
			};
			var alternate = ($$anchor) => {
				var fragment_1 = comment();
				each(first_child(fragment_1), 17, () => hunks(get(f).body), (h) => h.header, ($$anchor, h) => {
					var pre = root_6$4();
					var code_2 = child(pre);
					var span_3 = child(code_2);
					var text_14 = only_child(span_3, true);
					each(sibling(span_3, 2), 17, () => get(h).lines, index, ($$anchor, $$item) => {
						var $$array = /* @__PURE__ */ user_derived(() => to_array(get($$item), 2));
						let kind = () => get($$array)[0];
						let text = () => get($$array)[1];
						var fragment_2 = root_5$6();
						var span_4 = first_child(fragment_2);
						var text_15 = only_child(span_4);
						next();
						template_effect(() => {
							set_class(span_4, 1, clsx(kind()), "svelte-ap3eez");
							set_text(text_15, `${kind() === "added" ? "+" : kind() === "removed" ? "−" : " "}${text() ?? ""}`);
						});
						append($$anchor, fragment_2);
					});
					reset(code_2);
					reset(pre);
					template_effect(() => set_text(text_14, get(h).header));
					append($$anchor, pre);
				});
				append($$anchor, fragment_1);
			};
			if_block(node_4, ($$render) => {
				if ("binary" in get(f).body) $$render(consequent_3);
				else if ("skipped" in get(f).body) $$render(consequent_4, 1);
				else $$render(alternate, -1);
			});
			reset(article);
			template_effect(($0) => {
				set_text(text_8, get(f).path);
				set_text(text_9, $0);
				set_text(text_10, `+${get(f).added ?? ""}`);
				set_text(text_11, `−${get(f).removed ?? ""}`);
			}, [() => statusWord(get(f).status)]);
			append($$anchor, article);
		});
		var node_7 = sibling(node_3, 2);
		var consequent_5 = ($$anchor) => {
			var p_8 = root_8$3();
			var text_16 = child(p_8);
			var text_17 = only_child(sibling(text_16), true);
			reset(p_8);
			template_effect(() => {
				set_text(text_16, `Showing ${get(set$1).truncated.files_shown ?? ""} of ${get(set$1).truncated.files_total ?? ""} files.
        For all of it: `);
				set_text(text_17, get(set$1).truncated.command);
			});
			append($$anchor, p_8);
		};
		if_block(node_7, ($$render) => {
			if (get(set$1).truncated) $$render(consequent_5);
		});
		template_effect(($0) => {
			set_text(text_3, get(set$1).base);
			set_text(text_4, `+${get(totals).added ?? ""}`);
			set_text(text_5, `−${get(totals).removed ?? ""}`);
			set_text(text_6, ` · ${get(set$1).files.length ?? ""} ${$0 ?? ""}`);
		}, [() => plural(get(set$1).files.length, "file", "files")]);
		append($$anchor, fragment);
	};
	if_block(node_1, ($$render) => {
		if (get(loading)) $$render(consequent);
		else if (!get(chosen)) $$render(consequent_1, 1);
		else if (get(set$1)) $$render(consequent_6, 2);
	});
	reset(section);
	template_effect(() => {
		if (select_value !== (select_value = get(chosen))) select.value = (select.__value = select_value) ?? "", select_option(select, select_value);
		set_text(text_2, get(said));
	});
	delegated("change", select, (e) => load(e.currentTarget.value));
	append($$anchor, section);
	pop();
}
delegate(["change"]);
//#endregion
//#region src/surfaces/changes/index.ts
register({
	id: "changes",
	title: "What changed",
	band: "doing",
	order: 0,
	ports: [],
	select: (feed, focus) => {
		return {
			works: feed.board?.work ?? [],
			chosen: focus
		};
	},
	component: Changes
});
//#endregion
//#region src/surfaces/dispatch/Dispatch.svelte
var root$8 = /* @__PURE__ */ from_html(`<button type="button"> </button>`);
var root_1$8 = /* @__PURE__ */ from_html(`<fieldset class="svelte-7irco5"><legend class="svelte-7irco5">start from something you already wrote</legend> <!></fieldset>`);
var root_2$8 = /* @__PURE__ */ from_html(`<label class="svelte-7irco5"><input type="checkbox"/> </label>`);
var root_3$8 = /* @__PURE__ */ from_html(`<p class="dim svelte-7irco5">Choose a project and this says what will happen.</p>`);
var root_4$7 = /* @__PURE__ */ from_html(`<b class="refused svelte-7irco5"> </b> cannot take this.`, 1);
var root_5$5 = /* @__PURE__ */ from_html(`<li class="refused svelte-7irco5"><b> </b> </li>`);
var root_6$3 = /* @__PURE__ */ from_html(`<ul class="svelte-7irco5"></ul>`);
var root_7$2 = /* @__PURE__ */ from_html(`<p class="warn svelte-7irco5"><b> </b> </p>`);
var root_8$2 = /* @__PURE__ */ from_html(`<p><b> </b> <!></p> <!> <!>`, 1);
var root_9$2 = /* @__PURE__ */ from_html(`<section aria-labelledby="dispatch-head"><h2 id="dispatch-head" class="svelte-7irco5">Start work</h2> <textarea rows="3" placeholder="what should the agent do?" aria-label="what should the agent do?" class="svelte-7irco5"></textarea> <!> <fieldset class="svelte-7irco5"><legend class="svelte-7irco5">where</legend> <!></fieldset> <div class="will svelte-7irco5" aria-live="polite"><!></div></section>`);
function Dispatch($$anchor, $$props) {
	push($$props, true);
	const binding_group = [];
	let projects = prop($$props, "projects", 19, () => []), templates = prop($$props, "templates", 19, () => []), preflight = prop($$props, "preflight", 19, () => []), prompt = prop($$props, "prompt", 15, ""), chosen = prop($$props, "chosen", 31, () => proxy([]));
	const refusals = /* @__PURE__ */ user_derived(() => preflight().filter((p) => p.refusal !== null));
	const ready = /* @__PURE__ */ user_derived(() => preflight().filter((p) => p.refusal === null));
	const losing = /* @__PURE__ */ user_derived(() => preflight().filter((p) => (p.would_lose_fields ?? []).length > 0));
	const why = {
		untrusted: "not trusted yet — `devplane trust` it first",
		dirty_worktree: "has uncommitted changes, so an agent would build on them",
		no_such_agent: "names an agent this machine cannot start",
		config_will_not_load: "has a devplane.toml that will not parse",
		over_ceiling: "is already at the number of agents it allows at once"
	};
	var section = root_9$2();
	var textarea = sibling(child(section), 2);
	remove_textarea_child(textarea);
	var node = sibling(textarea, 2);
	var consequent = ($$anchor) => {
		var fieldset = root_1$8();
		each(sibling(child(fieldset), 2), 17, templates, (t) => t.id, ($$anchor, t) => {
			var button = root$8();
			var text = only_child(button, true);
			template_effect(() => set_text(text, get(t).title));
			append($$anchor, button);
		});
		reset(fieldset);
		append($$anchor, fieldset);
	};
	if_block(node, ($$render) => {
		if (templates().length > 0) $$render(consequent);
	});
	var fieldset_1 = sibling(node, 2);
	each(sibling(child(fieldset_1), 2), 17, projects, (p) => p.id, ($$anchor, p) => {
		var label = root_2$8();
		var input = child(label);
		remove_input_defaults(input);
		var input_value;
		var text_1 = sibling(input);
		reset(label);
		template_effect(() => {
			if (input_value !== (input_value = get(p).id)) input.value = (input.__value = input_value) ?? "";
			set_text(text_1, ` ${get(p).name ?? ""}`);
		});
		bind_group(binding_group, [], input, () => {
			get(p).id;
			return chosen();
		}, chosen);
		append($$anchor, label);
	});
	reset(fieldset_1);
	var div = sibling(fieldset_1, 2);
	var node_3 = child(div);
	var consequent_1 = ($$anchor) => {
		append($$anchor, root_3$8());
	};
	var alternate = ($$anchor) => {
		var fragment = root_8$2();
		var p_2 = first_child(fragment);
		var b = child(p_2);
		var text_2 = only_child(b, true);
		var text_3 = sibling(b);
		var node_4 = sibling(text_3);
		var consequent_2 = ($$anchor) => {
			var fragment_1 = root_4$7();
			var text_4 = only_child(first_child(fragment_1), true);
			next();
			template_effect(() => set_text(text_4, get(refusals).length));
			append($$anchor, fragment_1);
		};
		if_block(node_4, ($$render) => {
			if (get(refusals).length > 0) $$render(consequent_2);
		});
		reset(p_2);
		var node_5 = sibling(p_2, 2);
		var consequent_3 = ($$anchor) => {
			var ul = root_6$3();
			each(ul, 21, () => get(refusals), (r) => r.project, ($$anchor, r) => {
				var li = root_5$5();
				var b_2 = child(li);
				var text_5 = only_child(b_2, true);
				var text_6 = sibling(b_2);
				reset(li);
				template_effect(($0) => {
					set_text(text_5, $0);
					set_text(text_6, ` ${(get(r).refusal ? why[get(r).refusal] : "") ?? ""}`);
				}, [() => clip(String(get(r).project), 32)]);
				append($$anchor, li);
			});
			reset(ul);
			append($$anchor, ul);
		};
		if_block(node_5, ($$render) => {
			if (get(refusals).length > 0) $$render(consequent_3);
		});
		each(sibling(node_5, 2), 17, () => get(losing), (l) => l.project, ($$anchor, l) => {
			var p_3 = root_7$2();
			var b_3 = child(p_3);
			var text_7 = only_child(b_3, true);
			var text_8 = sibling(b_3);
			reset(p_3);
			template_effect(($0, $1) => {
				set_text(text_7, $0);
				set_text(text_8, ` would drop
          ${$1 ?? ""} — it still runs there.`);
			}, [() => clip(String(get(l).project), 32), () => (get(l).would_lose_fields ?? []).join(", ")]);
			append($$anchor, p_3);
		});
		template_effect(() => {
			set_text(text_2, get(ready).length);
			set_text(text_3, ` ${get(ready).length === 1 ? "project is" : "projects are"} ready. `);
		});
		append($$anchor, fragment);
	};
	if_block(node_3, ($$render) => {
		if (preflight().length === 0) $$render(consequent_1);
		else $$render(alternate, -1);
	});
	reset(div);
	reset(section);
	bind_value(textarea, prompt);
	append($$anchor, section);
	pop();
}
//#endregion
//#region src/surfaces/dispatch/index.ts
register({
	id: "dispatch",
	title: "Start work",
	band: "doing",
	order: 1,
	ports: ["dispatch"],
	select: (feed) => {
		return { projects: feed.board?.projects ?? [] };
	},
	component: Dispatch
});
//#endregion
//#region src/surfaces/github/Github.svelte
var root$7 = /* @__PURE__ */ from_html(`<p class="empty svelte-1n13jp3"> <code>gh</code>; a project with none configured is simply absent
      rather than empty.</p>`);
var root_1$7 = /* @__PURE__ */ from_html(`<span class="wait svelte-1n13jp3">needs you</span>`);
var root_2$7 = /* @__PURE__ */ from_html(`<li class="svelte-1n13jp3"><!> <span class="where svelte-1n13jp3"> </span> <a target="_blank" rel="noreferrer"> </a></li>`);
var root_3$7 = /* @__PURE__ */ from_html(`<ul role="list" class="svelte-1n13jp3"></ul>`);
var root_4$6 = /* @__PURE__ */ from_html(`<section aria-labelledby="gh-head"><h2 id="gh-head" class="svelte-1n13jp3">Issues and pull requests</h2> <div role="tablist" aria-label="what to show" class="svelte-1n13jp3"><button role="tab"> </button> <button role="tab"> </button></div> <!></section>`);
function Github($$anchor, $$props) {
	push($$props, true);
	let issues = prop($$props, "issues", 19, () => []), pulls = prop($$props, "pulls", 19, () => []), tab = prop($$props, "tab", 15, "issues");
	const shown = /* @__PURE__ */ user_derived(() => tab() === "issues" ? issues() : pulls());
	var section = root_4$6();
	var div = sibling(child(section), 2);
	var button = child(div);
	var text = only_child(button);
	var button_1 = sibling(button, 2);
	var text_1 = only_child(button_1);
	reset(div);
	var node = sibling(div, 2);
	var consequent = ($$anchor) => {
		var p = root$7();
		var text_2 = child(p);
		next(2);
		reset(p);
		template_effect(() => set_text(text_2, `Nothing open${tab() === "issues" ? "" : " that is waiting"} across your projects. Devplane reads
      the forge through your own `));
		append($$anchor, p);
	};
	var alternate = ($$anchor) => {
		var ul = root_3$7();
		each(ul, 21, () => get(shown), (r) => r.url, ($$anchor, r) => {
			var li = root_2$7();
			var node_1 = child(li);
			var consequent_1 = ($$anchor) => {
				append($$anchor, root_1$7());
			};
			if_block(node_1, ($$render) => {
				if (get(r).needs_you) $$render(consequent_1);
			});
			var span_1 = sibling(node_1, 2);
			var text_3 = only_child(span_1, true);
			var a = sibling(span_1, 2);
			var text_4 = only_child(a, true);
			reset(li);
			template_effect(($0) => {
				set_text(text_3, get(r).project);
				set_attribute(a, "href", get(r).url);
				set_text(text_4, $0);
			}, [() => clip(get(r).title, 90)]);
			append($$anchor, li);
		});
		reset(ul);
		append($$anchor, ul);
	};
	if_block(node, ($$render) => {
		if (get(shown).length === 0) $$render(consequent);
		else $$render(alternate, -1);
	});
	reset(section);
	template_effect(() => {
		set_attribute(button, "aria-selected", tab() === "issues");
		set_text(text, `issues (${issues().length ?? ""})`);
		set_attribute(button_1, "aria-selected", tab() === "pulls");
		set_text(text_1, `pull requests (${pulls().length ?? ""})`);
	});
	delegated("click", button, () => tab("issues"));
	delegated("click", button_1, () => tab("pulls"));
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/github/index.ts
register({
	id: "github",
	title: "Issues and pull requests",
	band: "doing",
	order: 2,
	ports: ["github"],
	select: () => ({}),
	component: Github
});
//#endregion
//#region src/surfaces/inbox/Inbox.svelte
var root$6 = /* @__PURE__ */ from_html(`<p class="hairline svelte-ytvk4v"> </p>`);
var root_1$6 = /* @__PURE__ */ from_html(`<button class="undo svelte-ytvk4v"> </button>`);
var root_2$6 = /* @__PURE__ */ from_html(`<p class="dim svelte-ytvk4v">Nothing needed you, and nothing was decided for you.</p>`);
var root_3$6 = /* @__PURE__ */ from_html(`<p class="dim svelte-ytvk4v"> </p>`);
var root_4$5 = /* @__PURE__ */ from_html(`<div class="close svelte-ytvk4v"><p class="svelte-ytvk4v"><b>Clear.</b></p> <!> <!> <!></div>`);
var root_5$4 = /* @__PURE__ */ from_html(`<span class="lvl svelte-ytvk4v"> </span>`);
var root_6$2 = /* @__PURE__ */ from_html(`<span class="sr svelte-ytvk4v"> </span>`);
var root_7$1 = /* @__PURE__ */ from_html(`<span class="unseen svelte-ytvk4v">new</span>`);
var root_8$1 = /* @__PURE__ */ from_html(`<span> </span>`);
var root_9$1 = /* @__PURE__ */ from_html(`<p class="d svelte-ytvk4v"> </p>`);
var root_10$1 = /* @__PURE__ */ from_html(`<button class="choice svelte-ytvk4v"> </button>`);
var root_11 = /* @__PURE__ */ from_html(`<span class="dead svelte-ytvk4v"> </span>`);
var root_12 = /* @__PURE__ */ from_html(`<div class="opts svelte-ytvk4v"></div>`);
var root_13 = /* @__PURE__ */ from_html(`<div class="reply svelte-ytvk4v"><input type="text" placeholder="your answer" class="svelte-ytvk4v"/> <button>reply</button></div>`);
var root_14 = /* @__PURE__ */ from_html(`<p class="offer svelte-ytvk4v"><span class="dim svelte-ytvk4v">never asked again:</span> <code class="svelte-ytvk4v"> </code> <button>copy</button> <span class="dim svelte-ytvk4v"> </span></p>`);
var root_15 = /* @__PURE__ */ from_html(`<button>allow</button>`);
var root_16 = /* @__PURE__ */ from_html(`<button>deny</button>`);
var root_17 = /* @__PURE__ */ from_html(`<button>snooze 1h</button>`);
var root_18 = /* @__PURE__ */ from_html(`<li><div class="t svelte-ytvk4v"><!> <b class="svelte-ytvk4v"> </b> <span class="kind svelte-ytvk4v"> </span> <!></div> <div class="meta svelte-ytvk4v"><!> <!></div> <!> <!> <!> <!> <div class="acts svelte-ytvk4v"><!> <!> <!></div></li>`);
var root_19 = /* @__PURE__ */ from_html(`<li class="item folded svelte-ytvk4v"><div class="t svelte-ytvk4v"><b class="svelte-ytvk4v"> </b></div> <div class="meta svelte-ytvk4v"><span> </span><span>folded — the list is long</span></div></li>`);
var root_20 = /* @__PURE__ */ from_html(`<li class="item inhibited svelte-ytvk4v"><div class="t svelte-ytvk4v"> </div> <div class="meta svelte-ytvk4v"><span> </span></div></li>`);
var root_21 = /* @__PURE__ */ from_html(`<ul role="list" class="svelte-ytvk4v"><!> <!> <!></ul>`);
var root_22 = /* @__PURE__ */ from_html(`<section aria-labelledby="inbox-head"><h2 id="inbox-head" class="svelte-ytvk4v">What needs you</h2> <!> <p class="said svelte-ytvk4v" role="status" aria-live="polite"> <!></p> <!></section>`);
function Inbox($$anchor, $$props) {
	push($$props, true);
	let items = prop($$props, "items", 19, () => []), folded = prop($$props, "folded", 19, () => []), inhibited = prop($$props, "inhibited", 19, () => []), close = prop($$props, "close", 3, null);
	let said = /* @__PURE__ */ state("");
	let undo = /* @__PURE__ */ state(null);
	async function answer(item, choice) {
		const ask = item.ask ?? item.request_id;
		if (!ask) {
			set(said, "this one cannot be answered from here");
			return;
		}
		try {
			await api(`/api/asks/${encodeURIComponent(ask)}/answer`, {
				method: "POST",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({ choice })
			});
			set(undo, null);
			set(said, "answered — on the record and on its way to the agent. That cannot be taken back.");
		} catch (e) {
			set(undo, null);
			set(said, `that did not land: ${e instanceof Error ? e.message : String(e)}`);
		}
	}
	let typed = proxy({});
	async function snooze(item) {
		const where = item.work_id ? `/api/work/${encodeURIComponent(item.work_id)}/snooze` : item.run_id ? `/api/runs/${encodeURIComponent(item.run_id)}/snooze` : item.project_id ? `/api/projects/${encodeURIComponent(item.project_id)}/snooze` : null;
		if (!where) {
			set(said, "there is nothing to snooze this against");
			return;
		}
		try {
			await api(`${where}?minutes=60`, { method: "POST" });
			set(said, "hidden for an hour");
			set(undo, {
				says: "put it back",
				where: `${where}?minutes=0`
			}, true);
		} catch (e) {
			set(undo, null);
			set(said, `that did not land: ${e instanceof Error ? e.message : String(e)}`);
		}
	}
	async function takeBack() {
		if (!get(undo)) return;
		try {
			await api(get(undo).where, { method: "POST" });
			set(said, "back in the list");
		} catch (e) {
			set(said, `that did not land: ${e instanceof Error ? e.message : String(e)}`);
		}
		set(undo, null);
	}
	async function copyRule(rule) {
		try {
			await navigator.clipboard?.writeText(rule);
			set(said, `copied: ${rule}`);
		} catch {
			set(said, "no clipboard here — select the rule above");
		}
	}
	const nothingRaised = /* @__PURE__ */ user_derived(() => items().length === 0 && folded().length === 0 && inhibited().length === 0);
	var section = root_22();
	var node = sibling(child(section), 2);
	var consequent = ($$anchor) => {
		var p = root$6();
		var text = only_child(p);
		template_effect(() => set_text(text, `since you last looked · ${close().since_last_look ?? ""}`));
		append($$anchor, p);
	};
	if_block(node, ($$render) => {
		if (close()?.since_last_look) $$render(consequent);
	});
	var p_1 = sibling(node, 2);
	var text_1 = child(p_1);
	var node_1 = sibling(text_1);
	var consequent_1 = ($$anchor) => {
		var button = root_1$6();
		var text_2 = only_child(button, true);
		template_effect(() => set_text(text_2, get(undo).says));
		delegated("click", button, takeBack);
		append($$anchor, button);
	};
	if_block(node_1, ($$render) => {
		if (get(undo)) $$render(consequent_1);
	});
	reset(p_1);
	var node_2 = sibling(p_1, 2);
	var consequent_5 = ($$anchor) => {
		var div = root_4$5();
		var node_3 = sibling(child(div), 2);
		var consequent_2 = ($$anchor) => {
			append($$anchor, root_2$6());
		};
		var alternate = ($$anchor) => {
			var fragment = comment();
			each(first_child(fragment), 16, () => close()?.sentences ?? [], (s) => s, ($$anchor, s) => {
				var p_3 = root_3$6();
				var text_3 = only_child(p_3, true);
				template_effect(() => set_text(text_3, s));
				append($$anchor, p_3);
			});
			append($$anchor, fragment);
		};
		if_block(node_3, ($$render) => {
			if (close()?.quiet) $$render(consequent_2);
			else $$render(alternate, -1);
		});
		var node_5 = sibling(node_3, 2);
		var consequent_3 = ($$anchor) => {
			var p_4 = root_3$6();
			var text_4 = only_child(p_4);
			template_effect(() => set_text(text_4, `next · ${close().next ?? ""}`));
			append($$anchor, p_4);
		};
		if_block(node_5, ($$render) => {
			if (close()?.next) $$render(consequent_3);
		});
		var node_6 = sibling(node_5, 2);
		var consequent_4 = ($$anchor) => {
			var p_5 = root_3$6();
			var text_5 = only_child(p_5, true);
			template_effect(() => set_text(text_5, close().keeps_running));
			append($$anchor, p_5);
		};
		if_block(node_6, ($$render) => {
			if (close()?.keeps_running) $$render(consequent_4);
		});
		reset(div);
		append($$anchor, div);
	};
	var alternate_3 = ($$anchor) => {
		var ul = root_21();
		var node_7 = child(ul);
		each(node_7, 17, items, (i) => i.id, ($$anchor, i) => {
			var li = root_18();
			var div_1 = child(li);
			var node_8 = child(div_1);
			var consequent_6 = ($$anchor) => {
				var span = root_5$4();
				var text_6 = only_child(span, true);
				template_effect(() => set_text(text_6, get(i).level));
				append($$anchor, span);
			};
			var alternate_1 = ($$anchor) => {
				var span_1 = root_6$2();
				var text_7 = only_child(span_1, true);
				template_effect(() => set_text(text_7, get(i).level));
				append($$anchor, span_1);
			};
			if_block(node_8, ($$render) => {
				if (get(i).level === "critical" || get(i).level === "high") $$render(consequent_6);
				else $$render(alternate_1, -1);
			});
			var b = sibling(node_8, 2);
			var text_8 = only_child(b, true);
			var span_2 = sibling(b, 2);
			var text_9 = only_child(span_2, true);
			var node_9 = sibling(span_2, 2);
			var consequent_7 = ($$anchor) => {
				append($$anchor, root_7$1());
			};
			if_block(node_9, ($$render) => {
				if (get(i).new_to_you) $$render(consequent_7);
			});
			reset(div_1);
			var div_2 = sibling(div_1, 2);
			var node_10 = child(div_2);
			var consequent_8 = ($$anchor) => {
				var span_4 = root_8$1();
				var text_10 = only_child(span_4, true);
				template_effect(() => set_text(text_10, get(i).project_name));
				append($$anchor, span_4);
			};
			if_block(node_10, ($$render) => {
				if (get(i).project_name) $$render(consequent_8);
			});
			var node_11 = sibling(node_10, 2);
			var consequent_9 = ($$anchor) => {
				var span_5 = root_8$1();
				var text_11 = only_child(span_5, true);
				template_effect(($0) => set_text(text_11, $0), [() => ago(Math.max(0, (Date.now() - Date.parse(get(i).since)) / 1e3))]);
				append($$anchor, span_5);
			};
			if_block(node_11, ($$render) => {
				if (get(i).since) $$render(consequent_9);
			});
			reset(div_2);
			var node_12 = sibling(div_2, 2);
			var consequent_10 = ($$anchor) => {
				var p_6 = root_9$1();
				var text_12 = only_child(p_6, true);
				template_effect(($0) => set_text(text_12, $0), [() => clip(get(i).detail, 400)]);
				append($$anchor, p_6);
			};
			if_block(node_12, ($$render) => {
				if (get(i).detail) $$render(consequent_10);
			});
			var node_13 = sibling(node_12, 2);
			var consequent_12 = ($$anchor) => {
				var div_3 = root_12();
				each(div_3, 21, () => get(i).options ?? [], (o) => o.label, ($$anchor, o) => {
					var fragment_1 = comment();
					var node_14 = first_child(fragment_1);
					var consequent_11 = ($$anchor) => {
						var button_1 = root_10$1();
						var text_13 = only_child(button_1, true);
						template_effect(() => set_text(text_13, get(o).label));
						delegated("click", button_1, () => answer(get(i), get(o).id));
						append($$anchor, button_1);
					};
					var alternate_2 = ($$anchor) => {
						var span_6 = root_11();
						var text_14 = only_child(span_6, true);
						template_effect(() => set_text(text_14, get(o).label));
						append($$anchor, span_6);
					};
					if_block(node_14, ($$render) => {
						if (get(o).id) $$render(consequent_11);
						else $$render(alternate_2, -1);
					});
					append($$anchor, fragment_1);
				});
				reset(div_3);
				append($$anchor, div_3);
			};
			var d = /* @__PURE__ */ user_derived(() => (get(i).options ?? []).length > 0 && (get(i).actions ?? []).includes("choose"));
			if_block(node_13, ($$render) => {
				if (get(d)) $$render(consequent_12);
			});
			var node_15 = sibling(node_13, 2);
			var consequent_13 = ($$anchor) => {
				var div_4 = root_13();
				var input = child(div_4);
				remove_input_defaults(input);
				var button_2 = sibling(input, 2);
				reset(div_4);
				template_effect(() => set_attribute(input, "aria-label", `your answer to: ${get(i).title ?? ""}`));
				bind_value(input, () => typed[get(i).id], ($$value) => typed[get(i).id] = $$value);
				delegated("click", button_2, () => answer(get(i), typed[get(i).id] ?? ""));
				append($$anchor, div_4);
			};
			var d_1 = /* @__PURE__ */ user_derived(() => (get(i).actions ?? []).includes("reply"));
			if_block(node_15, ($$render) => {
				if (get(d_1)) $$render(consequent_13);
			});
			var node_16 = sibling(node_15, 2);
			var consequent_14 = ($$anchor) => {
				var p_7 = root_14();
				var code = sibling(child(p_7), 2);
				var text_15 = only_child(code, true);
				var button_3 = sibling(code, 2);
				var text_16 = only_child(sibling(button_3, 2));
				reset(p_7);
				template_effect(() => {
					set_text(text_15, get(i).offer.rule);
					set_text(text_16, `paste into ${get(i).offer.file ?? ""} ${get(i).offer.section ?? ""} · covers ${get(i).offer.covers ?? ""}${get(i).offer.more ? "+" : ""} like it`);
				});
				delegated("click", button_3, () => copyRule(get(i).offer.rule));
				append($$anchor, p_7);
			};
			if_block(node_16, ($$render) => {
				if (get(i).offer) $$render(consequent_14);
			});
			var div_5 = sibling(node_16, 2);
			var node_17 = child(div_5);
			var consequent_15 = ($$anchor) => {
				var button_4 = root_15();
				delegated("click", button_4, () => answer(get(i), "allow"));
				append($$anchor, button_4);
			};
			var d_2 = /* @__PURE__ */ user_derived(() => (get(i).actions ?? []).includes("allow"));
			if_block(node_17, ($$render) => {
				if (get(d_2)) $$render(consequent_15);
			});
			var node_18 = sibling(node_17, 2);
			var consequent_16 = ($$anchor) => {
				var button_5 = root_16();
				delegated("click", button_5, () => answer(get(i), "deny"));
				append($$anchor, button_5);
			};
			var d_3 = /* @__PURE__ */ user_derived(() => (get(i).actions ?? []).includes("deny"));
			if_block(node_18, ($$render) => {
				if (get(d_3)) $$render(consequent_16);
			});
			var node_19 = sibling(node_18, 2);
			var consequent_17 = ($$anchor) => {
				var button_6 = root_17();
				delegated("click", button_6, () => snooze(get(i)));
				append($$anchor, button_6);
			};
			var d_4 = /* @__PURE__ */ user_derived(() => (get(i).actions ?? []).includes("snooze"));
			if_block(node_19, ($$render) => {
				if (get(d_4)) $$render(consequent_17);
			});
			reset(div_5);
			reset(li);
			template_effect(($0) => {
				set_class(li, 1, `item ${get(i).level ?? ""}`, "svelte-ytvk4v");
				set_text(text_8, get(i).title);
				set_text(text_9, $0);
			}, [() => get(i).kind.replace(/_/g, " ")]);
			append($$anchor, li);
		});
		var node_20 = sibling(node_7, 2);
		each(node_20, 17, folded, (f) => f.kind + (f.project ?? ""), ($$anchor, f) => {
			var li_1 = root_19();
			var div_6 = child(li_1);
			var text_17 = only_child(child(div_6));
			reset(div_6);
			var div_7 = sibling(div_6, 2);
			var text_18 = only_child(child(div_7), true);
			next();
			reset(div_7);
			reset(li_1);
			template_effect(() => {
				set_text(text_17, `${get(f).count ?? ""} × ${get(f).kind ?? ""}`);
				set_text(text_18, get(f).project ?? "across projects");
			});
			append($$anchor, li_1);
		});
		each(sibling(node_20, 2), 17, inhibited, (s) => s.cause, ($$anchor, s) => {
			var li_2 = root_20();
			var div_8 = child(li_2);
			var text_19 = only_child(div_8);
			var div_9 = sibling(div_8, 2);
			var text_20 = only_child(child(div_9), true);
			reset(div_9);
			reset(li_2);
			template_effect(($0) => {
				set_text(text_19, `${get(s).count ?? ""} more ${$0 ?? ""} counted here`);
				set_text(text_20, get(s).because);
			}, [() => plural(get(s).count, "item", "items")]);
			append($$anchor, li_2);
		});
		reset(ul);
		append($$anchor, ul);
	};
	if_block(node_2, ($$render) => {
		if (get(nothingRaised)) $$render(consequent_5);
		else $$render(alternate_3, -1);
	});
	reset(section);
	template_effect(() => set_text(text_1, `${get(said) ?? ""} `));
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/inbox/index.ts
register({
	id: "inbox",
	title: "What needs you",
	band: "attention",
	order: 0,
	ports: [
		"allow",
		"deny",
		"choose",
		"reply",
		"snooze",
		"copyrule"
	],
	select: (feed) => {
		const b = feed.inbox;
		return {
			items: b?.items ?? [],
			folded: b?.folded ?? [],
			inhibited: b?.inhibited ?? [],
			close: b?.close ?? null
		};
	},
	component: Inbox
});
//#endregion
//#region src/surfaces/search/Search.svelte
var root$5 = /* @__PURE__ */ from_html(`<p class="empty svelte-1acjvix">Nothing matched <b> </b>. Prompts and replies are not searched: Devplane
      never records them.</p>`);
var root_1$5 = /* @__PURE__ */ from_html(`<li class="svelte-1acjvix"><code> </code> <span> </span></li>`);
var root_2$5 = /* @__PURE__ */ from_html(`<ul role="list" class="svelte-1acjvix"></ul>`);
var root_3$5 = /* @__PURE__ */ from_html(`<section aria-labelledby="search-head"><h2 id="search-head" class="svelte-1acjvix">Search</h2> <form class="svelte-1acjvix"><input type="search" aria-label="search" placeholder="a command, a question, an error" class="svelte-1acjvix"/> <button type="submit">search</button></form> <p class="said svelte-1acjvix" role="status" aria-live="polite"> </p> <!></section>`);
function Search($$anchor, $$props) {
	push($$props, true);
	let query = /* @__PURE__ */ state("");
	let hits = /* @__PURE__ */ state(proxy([]));
	let ran = /* @__PURE__ */ state(false);
	let said = /* @__PURE__ */ state("");
	async function run() {
		const q = get(query).trim();
		if (!q) return;
		try {
			const r = await api(`/api/search?q=${encodeURIComponent(q)}`);
			set(hits, r.hits ?? [], true);
			set(ran, true);
			set(said, "");
		} catch (e) {
			set(said, `that did not land: ${e instanceof Error ? e.message : String(e)}`);
		}
	}
	var section = root_3$5();
	var form = sibling(child(section), 2);
	var input = child(form);
	remove_input_defaults(input);
	next(2);
	reset(form);
	var p = sibling(form, 2);
	var text = only_child(p, true);
	var node = sibling(p, 2);
	var consequent = ($$anchor) => {
		var p_1 = root$5();
		var text_1 = only_child(sibling(child(p_1)), true);
		next();
		reset(p_1);
		template_effect(($0) => set_text(text_1, $0), [() => clip(get(query), 40)]);
		append($$anchor, p_1);
	};
	var consequent_1 = ($$anchor) => {
		var ul = root_2$5();
		each(ul, 21, () => get(hits), (h) => h.run_id + h.at, ($$anchor, h) => {
			var li = root_1$5();
			var code = child(li);
			var text_2 = only_child(code, true);
			var text_3 = only_child(sibling(code, 2), true);
			reset(li);
			template_effect(($0, $1) => {
				set_text(text_2, $0);
				set_text(text_3, $1);
			}, [() => clip(get(h).run_id, 8), () => clip(get(h).text, 120)]);
			append($$anchor, li);
		});
		reset(ul);
		append($$anchor, ul);
	};
	if_block(node, ($$render) => {
		if (get(ran) && get(hits).length === 0) $$render(consequent);
		else if (get(hits).length > 0) $$render(consequent_1, 1);
	});
	reset(section);
	template_effect(() => set_text(text, get(said)));
	event("submit", form, (e) => {
		e.preventDefault();
		run();
	});
	bind_value(input, () => get(query), ($$value) => set(query, $$value));
	append($$anchor, section);
	pop();
}
//#endregion
//#region src/surfaces/search/index.ts
register({
	id: "search",
	title: "Search",
	band: "doing",
	order: 3,
	ports: ["search"],
	select: () => ({}),
	component: Search
});
//#endregion
//#region src/surfaces/setup/Setup.svelte
var root$4 = /* @__PURE__ */ from_html(`<p class="dim svelte-1y95tkv"> </p>`);
var root_1$4 = /* @__PURE__ */ from_html(`<p class="empty svelte-1y95tkv">Nothing is configured for this project yet. <code>devplane check</code> reads its <code>devplane.toml</code> and says what it will do.</p>`);
var root_2$4 = /* @__PURE__ */ from_html(`<dt class="svelte-1y95tkv"> </dt> <dd class="svelte-1y95tkv"> </dd>`, 1);
var root_3$4 = /* @__PURE__ */ from_html(`<dl class="svelte-1y95tkv"></dl>`);
var root_4$4 = /* @__PURE__ */ from_html(`<section aria-labelledby="setup-head"><h2 id="setup-head" class="svelte-1y95tkv">What is configured</h2> <!> <!> <p class="foot svelte-1y95tkv">Every value here is a file. Devplane reads them and never writes them: the rules are
    committed and reviewed like code, and an agent on this machine runs as you.</p></section>`);
function Setup($$anchor, $$props) {
	push($$props, true);
	let where = prop($$props, "where", 3, ""), rows = prop($$props, "rows", 19, () => []);
	var section = root_4$4();
	var node = sibling(child(section), 2);
	var consequent = ($$anchor) => {
		var p = root$4();
		var text = only_child(p, true);
		template_effect(() => set_text(text, where()));
		append($$anchor, p);
	};
	if_block(node, ($$render) => {
		if (where()) $$render(consequent);
	});
	var node_1 = sibling(node, 2);
	var consequent_1 = ($$anchor) => {
		append($$anchor, root_1$4());
	};
	var alternate = ($$anchor) => {
		var dl = root_3$4();
		each(dl, 21, rows, (r) => r.k, ($$anchor, r) => {
			var fragment = root_2$4();
			var dt = first_child(fragment);
			var text_1 = only_child(dt, true);
			var text_2 = only_child(sibling(dt, 2), true);
			template_effect(() => {
				set_text(text_1, get(r).k);
				set_text(text_2, get(r).v);
			});
			append($$anchor, fragment);
		});
		reset(dl);
		append($$anchor, dl);
	};
	if_block(node_1, ($$render) => {
		if (rows().length === 0) $$render(consequent_1);
		else $$render(alternate, -1);
	});
	next(2);
	reset(section);
	append($$anchor, section);
	pop();
}
//#endregion
//#region src/surfaces/setup/index.ts
register({
	id: "setup",
	title: "What is configured",
	band: "project",
	order: 0,
	ports: ["setup"],
	select: () => ({}),
	component: Setup
});
//#endregion
//#region src/surfaces/why/Why.svelte
var root$3 = /* @__PURE__ */ from_html(`<p class="dim svelte-1r7h8sn"> </p>`);
var root_1$3 = /* @__PURE__ */ from_html(`<p class="empty svelte-1r7h8sn">Open a row and this shows what was decided about it, and by whom.</p>`);
var root_2$3 = /* @__PURE__ */ from_html(`<p class="empty svelte-1r7h8sn">Nothing was decided about this. It is here because of what it is, not because of anything
      Devplane did.</p>`);
var root_3$3 = /* @__PURE__ */ from_html(`<span class="reason svelte-1r7h8sn"> </span>`);
var root_4$3 = /* @__PURE__ */ from_html(`<li class="svelte-1r7h8sn"><span class="at svelte-1r7h8sn"> </span> <span class="who svelte-1r7h8sn"> </span> <span class="did"> </span> <span class="outcome svelte-1r7h8sn"> </span> <!></li>`);
var root_5$3 = /* @__PURE__ */ from_html(`<ul role="list" class="svelte-1r7h8sn"></ul>`);
var root_6$1 = /* @__PURE__ */ from_html(`<section aria-labelledby="why-head"><h2 id="why-head" class="svelte-1r7h8sn">Why this is here</h2> <!> <p class="said svelte-1r7h8sn" role="status" aria-live="polite"> </p> <!></section>`);
function Why($$anchor, $$props) {
	push($$props, true);
	let about = prop($$props, "about", 3, ""), title = prop($$props, "title", 3, "");
	let rows = /* @__PURE__ */ state(proxy([]));
	let said = /* @__PURE__ */ state("");
	user_effect(() => {
		if (!about()) return;
		(async () => {
			try {
				set(rows, await api(`/api/decisions?about=${encodeURIComponent(about())}`), true);
				set(said, "");
			} catch (e) {
				set(said, `the decision log could not be read: ${e instanceof Error ? e.message : String(e)}`);
			}
		})();
	});
	var section = root_6$1();
	var node = sibling(child(section), 2);
	var consequent = ($$anchor) => {
		var p = root$3();
		var text = only_child(p, true);
		template_effect(() => set_text(text, title()));
		append($$anchor, p);
	};
	if_block(node, ($$render) => {
		if (title()) $$render(consequent);
	});
	var p_1 = sibling(node, 2);
	var text_1 = only_child(p_1, true);
	var node_1 = sibling(p_1, 2);
	var consequent_1 = ($$anchor) => {
		append($$anchor, root_1$3());
	};
	var consequent_2 = ($$anchor) => {
		append($$anchor, root_2$3());
	};
	var alternate = ($$anchor) => {
		var ul = root_5$3();
		each(ul, 21, () => get(rows), (d) => d.id, ($$anchor, d) => {
			var li = root_4$3();
			var span = child(li);
			var text_2 = only_child(span, true);
			var span_1 = sibling(span, 2);
			var text_3 = only_child(span_1, true);
			var span_2 = sibling(span_1, 2);
			var text_4 = only_child(span_2, true);
			var span_3 = sibling(span_2, 2);
			var text_5 = only_child(span_3, true);
			var node_2 = sibling(span_3, 2);
			var consequent_3 = ($$anchor) => {
				var span_4 = root_3$3();
				var text_6 = only_child(span_4, true);
				template_effect(() => set_text(text_6, get(d).reason));
				append($$anchor, span_4);
			};
			if_block(node_2, ($$render) => {
				if (get(d).reason) $$render(consequent_3);
			});
			reset(li);
			template_effect(($0) => {
				set_text(text_2, $0);
				set_text(text_3, get(d).authority);
				set_text(text_4, get(d).action);
				set_text(text_5, get(d).outcome);
			}, [() => clip(get(d).at, 19)]);
			append($$anchor, li);
		});
		reset(ul);
		append($$anchor, ul);
	};
	if_block(node_1, ($$render) => {
		if (!about()) $$render(consequent_1);
		else if (get(rows).length === 0 && !get(said)) $$render(consequent_2, 1);
		else $$render(alternate, -1);
	});
	reset(section);
	template_effect(() => set_text(text_1, get(said)));
	append($$anchor, section);
	pop();
}
//#endregion
//#region src/surfaces/why/index.ts
register({
	id: "why",
	title: "Why this is here",
	band: "attention",
	order: 3,
	ports: ["why"],
	select: (_feed, focus) => ({ about: focus }),
	component: Why
});
//#endregion
//#region src/surfaces/work/Certificate.svelte
var root$2 = /* @__PURE__ */ from_html(`<p class="dim svelte-17rlgir"> <!></p>`);
var root_1$2 = /* @__PURE__ */ from_html(`<dt class="svelte-17rlgir">warning</dt> <dd class="warn svelte-17rlgir"> </dd>`, 1);
var root_2$2 = /* @__PURE__ */ from_html(`<dt class="svelte-17rlgir">commit</dt><dd class="svelte-17rlgir"> </dd>`, 1);
var root_3$2 = /* @__PURE__ */ from_html(`<dt class="svelte-17rlgir">commit</dt><dd class="dim svelte-17rlgir"> </dd>`, 1);
var root_4$2 = /* @__PURE__ */ from_html(`<li class="svelte-17rlgir"><span aria-hidden="true"> </span> <span class="sr svelte-17rlgir"> </span> <code> </code> <span class="outcome svelte-17rlgir"> </span></li>`);
var root_5$2 = /* @__PURE__ */ from_html(`<dt class="svelte-17rlgir">origin</dt> <dd class="svelte-17rlgir"><code> </code></dd> <!> <!> <dt class="svelte-17rlgir">what was checked</dt> <dd class="svelte-17rlgir"><ul class="svelte-17rlgir"></ul></dd>`, 1);
var root_6 = /* @__PURE__ */ from_html(`<dt class="svelte-17rlgir">what was checked</dt><dd class="dim svelte-17rlgir"> </dd>`, 1);
var root_7 = /* @__PURE__ */ from_html(`<dt class="svelte-17rlgir">the agent said</dt> <dd class="svelte-17rlgir"><code> </code> <span class="claim"> </span> <p class="dim svelte-17rlgir"> </p></dd>`, 1);
var root_8 = /* @__PURE__ */ from_html(`<dt class="svelte-17rlgir">signed</dt> <dd class="dim svelte-17rlgir"> </dd>`, 1);
var root_9 = /* @__PURE__ */ from_html(`<dt class="svelte-17rlgir">what this does not establish</dt> <dd class="dim svelte-17rlgir"> </dd>`, 1);
var root_10 = /* @__PURE__ */ from_html(`<dl class="cert svelte-17rlgir"><dt class="svelte-17rlgir">basis</dt> <dd class="svelte-17rlgir"> </dd> <!> <!> <!> <!> <!> <!></dl>`);
function Certificate($$anchor, $$props) {
	push($$props, true);
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_1 = ($$anchor) => {
		var p = root$2();
		var text$1 = child(p);
		var node_1 = sibling(text$1);
		var consequent = ($$anchor) => {
			var text_1 = text();
			template_effect(() => set_text(text_1, `Its last gate said: ${$$props.page.last_gate ?? ""}`));
			append($$anchor, text_1);
		};
		if_block(node_1, ($$render) => {
			if ($$props.page.last_gate) $$render(consequent);
		});
		reset(p);
		template_effect(() => set_text(text$1, `${$$props.page.unfinished ?? ""} `));
		append($$anchor, p);
	};
	var alternate = ($$anchor) => {
		var dl = root_10();
		var dd = sibling(child(dl), 2);
		var text_2 = only_child(dd, true);
		var node_2 = sibling(dd, 2);
		var consequent_2 = ($$anchor) => {
			var fragment_2 = root_1$2();
			var text_3 = only_child(sibling(first_child(fragment_2), 2), true);
			template_effect(() => set_text(text_3, $$props.page.unchecked));
			append($$anchor, fragment_2);
		};
		if_block(node_2, ($$render) => {
			if ($$props.page.unchecked) $$render(consequent_2);
		});
		var node_3 = sibling(node_2, 2);
		var consequent_5 = ($$anchor) => {
			var fragment_3 = root_5$2();
			var dd_2 = sibling(first_child(fragment_3), 2);
			var text_4 = only_child(child(dd_2), true);
			reset(dd_2);
			var node_4 = sibling(dd_2, 2);
			var consequent_3 = ($$anchor) => {
				var fragment_4 = root_2$2();
				var text_5 = only_child(sibling(first_child(fragment_4)), true);
				template_effect(() => set_text(text_5, $$props.page.evidence.commit));
				append($$anchor, fragment_4);
			};
			if_block(node_4, ($$render) => {
				if ($$props.page.evidence.commit) $$render(consequent_3);
			});
			var node_5 = sibling(node_4, 2);
			var consequent_4 = ($$anchor) => {
				var fragment_5 = root_3$2();
				var text_6 = only_child(sibling(first_child(fragment_5)), true);
				template_effect(() => set_text(text_6, $$props.page.evidence.no_commit));
				append($$anchor, fragment_5);
			};
			if_block(node_5, ($$render) => {
				if ($$props.page.evidence.no_commit) $$render(consequent_4);
			});
			var dd_5 = sibling(node_5, 4);
			var ul = child(dd_5);
			each(ul, 21, () => $$props.page.evidence.commands, (c) => c.command, ($$anchor, c) => {
				var li = root_4$2();
				var span = child(li);
				var text_7 = only_child(span, true);
				var span_1 = sibling(span, 2);
				var text_8 = only_child(span_1, true);
				var code_1 = sibling(span_1, 2);
				var text_9 = only_child(code_1, true);
				var text_10 = only_child(sibling(code_1, 2), true);
				reset(li);
				template_effect(() => {
					set_class(span, 1, `mark ${get(c).passed ? "ok" : "bad"}`, "svelte-17rlgir");
					set_text(text_7, get(c).passed ? "✓" : "✗");
					set_text(text_8, get(c).passed ? "met" : "did not meet");
					set_attribute(code_1, "title", get(c).command);
					set_text(text_9, get(c).shown);
					set_text(text_10, get(c).outcome);
				});
				append($$anchor, li);
			});
			reset(ul);
			reset(dd_5);
			template_effect(() => set_text(text_4, $$props.page.evidence["gen_ai.evidence.origin"]));
			append($$anchor, fragment_3);
		};
		if_block(node_3, ($$render) => {
			if ($$props.page.evidence) $$render(consequent_5);
		});
		var node_6 = sibling(node_3, 2);
		var consequent_6 = ($$anchor) => {
			var fragment_6 = root_6();
			var text_11 = only_child(sibling(first_child(fragment_6)), true);
			template_effect(() => set_text(text_11, $$props.page.no_evidence));
			append($$anchor, fragment_6);
		};
		if_block(node_6, ($$render) => {
			if ($$props.page.no_evidence) $$render(consequent_6);
		});
		var node_7 = sibling(node_6, 2);
		var consequent_7 = ($$anchor) => {
			var fragment_7 = root_7();
			var dd_7 = sibling(first_child(fragment_7), 2);
			var code_2 = child(dd_7);
			var text_12 = only_child(code_2, true);
			var span_3 = sibling(code_2, 2);
			var text_13 = only_child(span_3, true);
			var text_14 = only_child(sibling(span_3, 2), true);
			reset(dd_7);
			template_effect(($0) => {
				set_text(text_12, $$props.page.claim["gen_ai.evidence.origin"]);
				set_text(text_13, $0);
				set_text(text_14, $$props.page.claim.caveat);
			}, [() => clip($$props.page.claim.text, 400)]);
			append($$anchor, fragment_7);
		};
		if_block(node_7, ($$render) => {
			if ($$props.page.claim) $$render(consequent_7);
		});
		var node_8 = sibling(node_7, 2);
		var consequent_8 = ($$anchor) => {
			var fragment_8 = root_8();
			var text_15 = only_child(sibling(first_child(fragment_8), 2), true);
			template_effect(() => set_text(text_15, $$props.page.signing.says));
			append($$anchor, fragment_8);
		};
		if_block(node_8, ($$render) => {
			if ($$props.page.signing) $$render(consequent_8);
		});
		var node_9 = sibling(node_8, 2);
		var consequent_9 = ($$anchor) => {
			var fragment_9 = root_9();
			var text_16 = only_child(sibling(first_child(fragment_9), 2), true);
			template_effect(() => set_text(text_16, $$props.page.limits));
			append($$anchor, fragment_9);
		};
		if_block(node_9, ($$render) => {
			if ($$props.page.limits) $$render(consequent_9);
		});
		reset(dl);
		template_effect(() => set_text(text_2, $$props.page.basis));
		append($$anchor, dl);
	};
	if_block(node, ($$render) => {
		if (!$$props.page.finished) $$render(consequent_1);
		else $$render(alternate, -1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
//#region src/surfaces/work/Work.svelte
var root$1 = /* @__PURE__ */ from_html(`<button class="primary">release this step</button>`);
var root_1$1 = /* @__PURE__ */ from_html(`<button>pick it back up</button> <button>try again</button>`, 1);
var root_2$1 = /* @__PURE__ */ from_html(`<p class="dim svelte-17kmig3">No work has been started here. <code>devplane work start "…"</code> makes one — an isolated
      checkout, the project's own checks, and a certificate when it finishes.</p>`);
var root_3$1 = /* @__PURE__ */ from_html(`<p class="dim svelte-17kmig3">Reading the evidence…</p>`);
var root_4$1 = /* @__PURE__ */ from_html(`<p class="dim svelte-17kmig3">There is no evidence to read for this one.</p>`);
var root_5$1 = /* @__PURE__ */ from_html(`<section aria-labelledby="work-head"><h2 id="work-head" class="svelte-17kmig3"> </h2> <p class="said svelte-17kmig3" role="status" aria-live="polite"> </p> <div class="acts svelte-17kmig3"><!> <!></div> <!></section>`);
function Work($$anchor, $$props) {
	push($$props, true);
	let title = prop($$props, "title", 3, ""), id = prop($$props, "id", 3, ""), phase = prop($$props, "phase", 3, "");
	let said = /* @__PURE__ */ state("");
	let page = /* @__PURE__ */ state(null);
	let reading = /* @__PURE__ */ state(false);
	user_effect(() => {
		const want = id();
		set(page, null);
		if (!want) return;
		set(reading, true);
		(async () => {
			try {
				set(page, await api(`/api/work/${encodeURIComponent(want)}/certificate`), true);
				set(said, "");
			} catch (e) {
				set(said, `the evidence could not be read: ${e instanceof Error ? e.message : String(e)}`);
			} finally {
				set(reading, false);
			}
		})();
	});
	async function act(verb) {
		if (!id()) {
			set(said, "there is no work here to act on");
			return;
		}
		const where = {
			approve: `/api/work/${encodeURIComponent(id())}/approve`,
			resume: `/api/work/${encodeURIComponent(id())}/resume`,
			retry: `/api/work/${encodeURIComponent(id())}/retry`
		}[verb];
		try {
			await api(where, { method: "POST" });
			set(said, {
				approve: "released — the pipeline continues, and the decision is recorded as yours",
				resume: "picked back up",
				retry: "trying again"
			}[verb], true);
		} catch (e) {
			set(said, `that did not land: ${e instanceof Error ? e.message : String(e)}`);
		}
	}
	var section = root_5$1();
	var h2 = child(section);
	var text = only_child(h2, true);
	var p = sibling(h2, 2);
	var text_1 = only_child(p, true);
	var div = sibling(p, 2);
	var node = child(div);
	var consequent = ($$anchor) => {
		var button = root$1();
		delegated("click", button, () => act("approve"));
		append($$anchor, button);
	};
	if_block(node, ($$render) => {
		if (phase() === "human") $$render(consequent);
	});
	var node_1 = sibling(node, 2);
	var consequent_1 = ($$anchor) => {
		var fragment = root_1$1();
		var button_1 = first_child(fragment);
		var button_2 = sibling(button_1, 2);
		delegated("click", button_1, () => act("resume"));
		delegated("click", button_2, () => act("retry"));
		append($$anchor, fragment);
	};
	if_block(node_1, ($$render) => {
		if (phase() === "stopped" || phase() === "failed") $$render(consequent_1);
	});
	reset(div);
	var node_2 = sibling(div, 2);
	var consequent_2 = ($$anchor) => {
		append($$anchor, root_2$1());
	};
	var consequent_3 = ($$anchor) => {
		append($$anchor, root_3$1());
	};
	var consequent_4 = ($$anchor) => {
		append($$anchor, root_4$1());
	};
	var alternate = ($$anchor) => {
		Certificate($$anchor, { get page() {
			return get(page);
		} });
	};
	if_block(node_2, ($$render) => {
		if (!id()) $$render(consequent_2);
		else if (get(reading)) $$render(consequent_3, 1);
		else if (get(page) === null) $$render(consequent_4, 2);
		else $$render(alternate, -1);
	});
	reset(section);
	template_effect(() => {
		set_text(text, title() || "Work");
		set_text(text_1, get(said));
	});
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/work/index.ts
register({
	id: "work",
	title: "Is this actually done",
	band: "attention",
	order: 2,
	ports: [
		"approve",
		"resume",
		"retry"
	],
	select: (feed, focus) => {
		const all = feed.board?.work ?? [];
		const pick = all.find((w) => w.id === focus) ?? all[0];
		return {
			id: pick?.id ?? "",
			title: pick?.title ?? "",
			phase: pick?.phase ?? "",
			all
		};
	},
	component: Work
});
//#endregion
//#region src/lib/live.svelte.ts
var EVERY_MS = 2e3;
function live() {
	const state = proxy({
		board: null,
		inbox: null,
		error: null,
		unauthorised: false
	});
	async function tick(read) {
		try {
			const [board, inbox] = await Promise.all([api("/api/board"), api(read ? "/api/inbox?read=true" : "/api/inbox")]);
			state.board = board;
			state.inbox = inbox;
			state.error = null;
			state.unauthorised = false;
		} catch (e) {
			if (e instanceof Unauthorised) {
				state.unauthorised = true;
				state.error = e.message;
				return;
			}
			state.error = e instanceof Error ? e.message : String(e);
		}
	}
	function start() {
		tick(true);
		const id = setInterval(() => void tick(false), EVERY_MS);
		const onVisible = () => {
			if (document.visibilityState === "visible") tick(true);
		};
		document.addEventListener("visibilitychange", onVisible);
		return () => {
			clearInterval(id);
			document.removeEventListener("visibilitychange", onVisible);
		};
	}
	return {
		state,
		start
	};
}
//#endregion
//#region src/lib/theme.svelte.ts
var KEY = "devplane_theme";
function read() {
	try {
		const v = localStorage.getItem(KEY);
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
			if (next === "system") localStorage.removeItem(KEY);
			else localStorage.setItem(KEY, next);
		} catch {}
	}
	return {
		state,
		restore: () => apply(state.choice),
		cycle: () => apply(state.choice === "system" ? "light" : state.choice === "light" ? "dark" : "system")
	};
}
//#endregion
//#region src/App.svelte
var root = /* @__PURE__ */ from_html(`<li><button class="tab svelte-1n46o8q"> </button></li>`);
var root_1 = /* @__PURE__ */ from_html(`<ul role="list" class="svelte-1n46o8q"></ul>`);
var root_2 = /* @__PURE__ */ from_html(`<p class="problem svelte-1n46o8q" role="alert">This tab has no token. Run <code>devplane open</code> again to get a fresh link.</p>`);
var root_3 = /* @__PURE__ */ from_html(`<p class="problem svelte-1n46o8q" role="status"> </p>`);
var root_4 = /* @__PURE__ */ from_html(`<p>No surface is registered.</p>`);
var root_5 = /* @__PURE__ */ from_html(`<a class="skip svelte-1n46o8q" href="#surface">Skip to content</a> <div class="app svelte-1n46o8q"><aside class="svelte-1n46o8q"><div class="mark svelte-1n46o8q"><b class="svelte-1n46o8q">Devplane</b> <span class="what svelte-1n46o8q">who decided, when nobody asked you</span></div> <nav aria-label="surfaces" class="svelte-1n46o8q"></nav> <div class="foot svelte-1n46o8q"><span role="status"><span class="dot svelte-1n46o8q" aria-hidden="true"></span> </span> <button class="theme svelte-1n46o8q"> </button></div></aside> <main id="surface" class="svelte-1n46o8q"><!> <!></main></div>`, 1);
function App($$anchor, $$props) {
	push($$props, true);
	const { state: feed, start } = live();
	user_effect(start);
	const { state: themeState, restore, cycle } = theme();
	user_effect(restore);
	let current = /* @__PURE__ */ state(proxy(landing()?.id ?? ""));
	let focus = /* @__PURE__ */ state("");
	const showing = /* @__PURE__ */ user_derived(() => surfaces().find((s) => s.id === get(current)));
	function read() {
		const raw = location.hash.slice(1);
		const cut = raw.indexOf("/");
		const id = cut === -1 ? raw : raw.slice(0, cut);
		if (!surfaces().some((s) => s.id === id)) return;
		set(current, id, true);
		set(focus, cut === -1 ? "" : decodeURIComponent(raw.slice(cut + 1)), true);
	}
	user_effect(() => {
		read();
		addEventListener("hashchange", read);
		return () => removeEventListener("hashchange", read);
	});
	function go(id) {
		set(current, id, true);
		set(focus, "");
		history.replaceState({}, "", `#${id}`);
	}
	function keepInView(node, isCurrent) {
		const show = (on) => {
			if (!on) return;
			node.scrollIntoView({
				block: "nearest",
				inline: "nearest"
			});
		};
		show(isCurrent);
		return { update: show };
	}
	const banded = /* @__PURE__ */ user_derived(() => BANDS.map((band) => ({
		band,
		items: surfaces().filter((s) => s.band === band)
	})).filter((g) => g.items.length > 0));
	const themeLabel = /* @__PURE__ */ user_derived(() => themeState.choice === "system" ? "following your system" : `${themeState.choice} theme`);
	var fragment = root_5();
	var div = sibling(first_child(fragment), 2);
	var aside = child(div);
	var nav = sibling(child(aside), 2);
	each(nav, 21, () => get(banded), (g) => g.band, ($$anchor, g) => {
		var ul = root_1();
		each(ul, 21, () => get(g).items, (s) => s.id, ($$anchor, s) => {
			var li = root();
			var button = child(li);
			var text = only_child(button, true);
			action(button, ($$node, $$action_arg) => keepInView?.($$node, $$action_arg), () => get(s).id === get(current));
			reset(li);
			template_effect(() => {
				set_attribute(button, "aria-current", get(s).id === get(current) ? "page" : void 0);
				set_text(text, get(s).title);
			});
			delegated("click", button, () => go(get(s).id));
			append($$anchor, li);
		});
		reset(ul);
		append($$anchor, ul);
	});
	reset(nav);
	var div_1 = sibling(nav, 2);
	var span = child(div_1);
	let classes;
	var text_1 = sibling(child(span));
	reset(span);
	var button_1 = sibling(span, 2);
	var text_2 = only_child(button_1, true);
	reset(div_1);
	reset(aside);
	var main = sibling(aside, 2);
	var node_1 = child(main);
	var consequent = ($$anchor) => {
		append($$anchor, root_2());
	};
	var consequent_1 = ($$anchor) => {
		var p_1 = root_3();
		var text_3 = only_child(p_1);
		template_effect(() => set_text(text_3, `Devplane could not be reached: ${feed.error ?? ""}`));
		append($$anchor, p_1);
	};
	if_block(node_1, ($$render) => {
		if (feed.unauthorised) $$render(consequent);
		else if (feed.error) $$render(consequent_1, 1);
	});
	var node_2 = sibling(node_1, 2);
	var consequent_2 = ($$anchor) => {
		var fragment_1 = comment();
		var node_3 = first_child(fragment_1);
		{
			let $0 = /* @__PURE__ */ user_derived(() => get(showing).select(feed, get(focus)));
			component(node_3, () => get(showing).component, ($$anchor, showing_component) => {
				showing_component($$anchor, spread_props(() => get($0)));
			});
		}
		append($$anchor, fragment_1);
	};
	var alternate = ($$anchor) => {
		append($$anchor, root_4());
	};
	if_block(node_2, ($$render) => {
		if (get(showing)) $$render(consequent_2);
		else $$render(alternate, -1);
	});
	reset(main);
	reset(div);
	template_effect(() => {
		classes = set_class(span, 1, "pulse svelte-1n46o8q", null, classes, { bad: !!feed.error || feed.unauthorised });
		set_text(text_1, ` ${feed.unauthorised ? "no token" : feed.error ? "not answering" : "live"}`);
		set_attribute(button_1, "title", `Theme: ${get(themeLabel) ?? ""}`);
		set_attribute(button_1, "aria-label", `Theme: ${get(themeLabel) ?? ""}`);
		set_text(text_2, themeState.choice === "system" ? "auto" : themeState.choice);
	});
	delegated("click", button_1, cycle);
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