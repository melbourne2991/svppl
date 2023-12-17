import { task, Type } from "./lib.ts";

export const myTask = task(
  {
    id: "my-task",
    name: "My Task",
    description: "This is my task",
    parameters: {
      foo: Type.String(),
    },
  },
  async ({ foo }) => {
    return foo;
  }
);

