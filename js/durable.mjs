function instance(events = []) {
  let count = 0;
  
  const yielded = {
    current: null
  };

  function task(name, fn) {
    return (...args) => {
      console.log(events)
      if (events[count]) {
        return events[count]
      }

      count++;

      return new Promise((resolve, reject) => {
        reject({name, args, count});
      });
    };
  }

  async function durable(fn) {
    try {
      await fn();
    } catch (ev) {
      yielded.current = ev;
    }
  }

  return {
    task,
    durable,
    yielded
  };
}

let i = instance();
let add = i.task("myTask", async () => {});

let runner = async () => {
  let x = await add(5, 3);
  let y = await add(x, 3);
  console.log('got y', y)
};

await i.durable(runner);

i = instance([
  i.yielded.current.args.reduce((a, b) => a + b, 0),
])
add = i.task("myTask", async () => {});

await i.durable(runner);

console.log(i.yielded.current);