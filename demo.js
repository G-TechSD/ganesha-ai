async function example() {
  const data = await Promise.resolve('Hello');
  console.log(data);
}
example();
