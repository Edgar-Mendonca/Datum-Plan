// Task Path (Schedule › Paths): for the selected task, colour its driving predecessors, other
// predecessors, driven successors and other successors in the task sheet and the Gantt.
// The engine works the path out (sched::task_path); the window only shows it.
import { createSignal, createEffect, createRoot } from "solid-js";
import { call, out, pane, sel } from "./store";

/** 1 the task, 2 driving predecessor, 3 predecessor, 4 driven successor, 5 successor. */
export const [pathOn, setPathOn] = pane("task-path", false);
export const [pathMap, setPathMap] = createSignal<Map<number, number>>(new Map());
export const PATH: [number, string, string][] = [[2, "Driving predecessors", "rgba(217,119,6,.30)"], [3, "Predecessors", "rgba(217,119,6,.11)"], [4, "Driven successors", "rgba(124,58,237,.24)"], [5, "Successors", "rgba(124,58,237,.09)"]];
export const pathColor = (k: number | undefined) => PATH.find(p => p[0] === k)?.[2];

createRoot(() => createEffect(() => {
  const on = pathOn(), u = sel()[0];
  out();
  if (!on || !u) { setPathMap(new Map()); return; }
  call<[number, number][]>("task_path", { uid: u }).then(v => setPathMap(new Map(v ?? [])));
}));
