// process
if ('production' !== process.env.NODE_ENV && process) {
  console.log(process.env.NODE_ENV, process);
} else {
  console.log('HAHA');
}
