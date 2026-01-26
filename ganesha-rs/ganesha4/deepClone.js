function deepClone(obj) {
  if (typeof obj !== "object" || obj === null) {
    return obj;
  }

  let clonedObj = Array.isArray(obj) ? [] : {};

  for (let key in obj) {
    if (obj.hasOwnProperty(key)) {
      clonedObj[key] = deepClone(obj[key]);
    }
  }

  return clonedObj;
}

// Example usage:
const originalObject = {
  name: "John Doe",
  age: 30,
  address: {
    street: "123 Main St",
    city: "Anytown"
  },
  hobbies: ["reading", "hiking"]
};

const clonedObject = deepClone(originalObject);

// Modify the cloned object
clonedObject.address.city = "New City";
clonedObject.hobbies.push("coding");

console.log("Original Object:", originalObject);
console.log("Cloned Object:", clonedObject);
