import { Type, TSchema, Static } from "@sinclair/typebox";

export type TaskParameters = Record<string, TSchema>;

export interface TaskConfig<P extends TaskParameters> {
  id: string;
  name: string;
  description: string;
  parameters: P;
}

export type TaskFunctionImpl<P extends TaskParameters> = (
  params: InferParameters<P>
) => unknown | Promise<unknown>;

type InferParameters<P extends TaskParameters> = {
  [K in keyof P]: P[K] extends TSchema ? Static<P[K]> : never;
};

export interface TaskDefinition<P extends TaskParameters = TaskParameters> {
  config: TaskConfig<P>;
  exec: TaskFunctionImpl<P>;
}

const taskRegistry = new Map<string, TaskDefinition>();

export function task<P extends Record<string, TSchema>>(
  config: TaskConfig<P>,
  exec: TaskFunctionImpl<P>
) {
  if (taskRegistry.has(config.id)) {
    throw new Error(`Task ${config.id} already exists in this namespace`);
  }

  taskRegistry.set(config.id, {
    config,
    exec: exec as TaskFunctionImpl<TaskParameters>,
  });
}

export { Type };
