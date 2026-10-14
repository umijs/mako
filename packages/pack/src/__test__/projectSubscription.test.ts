import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Project } from "../core/types";

const bindingMocks = vi.hoisted(() => ({
  endpointClientChangedSubscribe: vi.fn(),
  endpointServerChangedSubscribe: vi.fn(),
  projectEntrypointsSubscribe: vi.fn(),
  projectHmrEvents: vi.fn(),
  projectHmrIdentifiersSubscribe: vi.fn(),
  projectUpdateInfoSubscribe: vi.fn(),
  projectNew: vi.fn(),
  registerWorkerScheduler: undefined,
  rootTaskDispose: vi.fn(),
}));

vi.mock("../binding", () => bindingMocks);

import { projectFactory, TurbopackInternalError } from "../core/project";

const task = { __napiType: "RootTask" as const };
const done = { value: undefined, done: true };
type Callback = (error: Error | undefined, value?: unknown) => void;

const subscriptions = [
  {
    name: "entrypoints",
    binding: bindingMocks.projectEntrypointsSubscribe,
    open: (project: Project) => project.entrypointsSubscribe(),
  },
  {
    name: "HMR events",
    binding: bindingMocks.projectHmrEvents,
    open: (project: Project) => project.hmrEvents("entry"),
  },
  {
    name: "HMR identifiers",
    binding: bindingMocks.projectHmrIdentifiersSubscribe,
    open: (project: Project) => project.hmrIdentifiersSubscribe(),
  },
  {
    name: "update info",
    binding: bindingMocks.projectUpdateInfoSubscribe,
    open: (project: Project) => project.updateInfoSubscribe(0),
  },
];

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

async function createProject() {
  return projectFactory()(
    {
      rootPath: "/project",
      projectPath: "/project",
      packPath: "/pack",
      config: { entry: [], output: {} },
      dev: true,
      watch: { enable: true },
      processEnv: {},
    } as never,
    {},
  );
}

beforeEach(() => {
  vi.resetAllMocks();
  bindingMocks.projectNew.mockResolvedValue({ __napiType: "Project" });
});

describe.each(subscriptions)(
  "$name subscription cleanup",
  ({ binding, open }) => {
    function startWithResult() {
      let emit!: Callback;
      binding.mockImplementation((...args: unknown[]) => {
        emit = args.find((arg) => typeof arg === "function") as Callback;
        emit(undefined, { issues: [] });
        return Promise.resolve(task);
      });
      return (error?: Error, value?: unknown) => emit(error, value);
    }

    it("interrupts pending next calls and disposes before return resolves", async () => {
      const emit = startWithResult();
      const subscription = open(await createProject());
      await subscription.next();
      const next = subscription.next();
      const queuedNext = subscription.next();

      await expect(subscription.return!()).resolves.toEqual(done);

      expect(bindingMocks.rootTaskDispose.mock.calls).toEqual([[task]]);
      await expect(next).resolves.toEqual(done);
      await expect(queuedNext).resolves.toEqual(done);
      emit(new Error("late native callback"));
      await expect(subscription.next()).resolves.toEqual(done);
    });

    it("disposes when paused after a yield and tolerates repeated return", async () => {
      startWithResult();
      const subscription = open(await createProject());
      await subscription.next();

      await expect(
        Promise.all([subscription.return!(), subscription.return!()]),
      ).resolves.toEqual([done, done]);

      expect(bindingMocks.rootTaskDispose.mock.calls).toEqual([[task]]);
      await expect(subscription.return!()).resolves.toEqual(done);
      await expect(subscription.next()).resolves.toEqual(done);
    });

    it("closes without starting a native subscription", async () => {
      const subscription = open(await createProject());

      await expect(subscription.return!()).resolves.toEqual(done);
      await expect(subscription.next()).resolves.toEqual(done);

      expect(binding).not.toHaveBeenCalled();
      expect(bindingMocks.rootTaskDispose).not.toHaveBeenCalled();
    });

    it("waits for native registration and disposes a late root task", async () => {
      const registration = deferred<typeof task>();
      let emit!: Callback;
      binding.mockImplementation((...args: unknown[]) => {
        emit = args.find((arg) => typeof arg === "function") as Callback;
        return registration.promise;
      });
      const subscription = open(await createProject());
      const next = subscription.next();
      let returned = false;
      const closing = subscription.return!().then((result) => {
        returned = true;
        return result;
      });
      await Promise.resolve();

      expect(returned).toBe(false);
      expect(bindingMocks.rootTaskDispose).not.toHaveBeenCalled();
      emit(undefined, { issues: [] });
      registration.resolve(task);

      await expect(closing).resolves.toEqual(done);
      await expect(next).resolves.toEqual(done);
      expect(bindingMocks.rootTaskDispose.mock.calls).toEqual([[task]]);
    });

    it("preserves native registration errors during cancellation", async () => {
      const registration = deferred<typeof task>();
      binding.mockReturnValue(registration.promise);
      const subscription = open(await createProject());
      const next = subscription.next();
      const rejected = expect(next).rejects.toMatchObject({
        name: "TurbopackInternalError",
        message: "registration failed",
      });
      const closing = subscription.return!();
      registration.reject(new Error("registration failed"));

      await rejected;
      await expect(closing).resolves.toEqual(done);
      expect(bindingMocks.rootTaskDispose).not.toHaveBeenCalled();
    });

    it("wraps native callback errors and disposes the root task", async () => {
      const emit = startWithResult();
      const subscription = open(await createProject());
      await subscription.next();
      const next = subscription.next();
      emit(new Error("subscription failed"));

      await expect(next).rejects.toBeInstanceOf(TurbopackInternalError);
      expect(bindingMocks.rootTaskDispose.mock.calls).toEqual([[task]]);
      await expect(subscription.return!()).resolves.toEqual(done);
    });
  },
);

describe.each(["client", "server"] as const)(
  "%s endpoint subscription cleanup",
  (direction) => {
    it("disposes a subscription primed by the endpoint method", async () => {
      const project = await createProject();
      bindingMocks.projectEntrypointsSubscribe.mockImplementation(
        (_project, callback: Callback) => {
          callback(undefined, {
            apps: [{ __napiType: "Endpoint" }],
            issues: [],
          });
          return Promise.resolve();
        },
      );
      const entrypoints = project.entrypointsSubscribe();
      const { value } = await entrypoints.next();
      await entrypoints.return!();
      const endpoint = value.apps![0];
      const binding =
        direction === "client"
          ? bindingMocks.endpointClientChangedSubscribe
          : bindingMocks.endpointServerChangedSubscribe;
      binding.mockImplementation((...args: unknown[]) => {
        const emit = args.find((arg) => typeof arg === "function") as Callback;
        emit(undefined, { issues: [] });
        return Promise.resolve(task);
      });
      const subscription =
        direction === "client"
          ? await endpoint.clientChanged()
          : await endpoint.serverChanged(true);

      await expect(subscription.return!()).resolves.toEqual(done);

      expect(bindingMocks.rootTaskDispose.mock.calls).toEqual([[task]]);
    });
  },
);
