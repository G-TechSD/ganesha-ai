class Singleton {
  private static instance: Singleton;

  private constructor() {
    // Private constructor to prevent direct instantiation
  }

  public static getInstance(): Singleton {
    if (!Singleton.instance) {
      Singleton.instance = new Singleton();
    }
    return Singleton.instance;
  }

  public someBusinessLogic(): void {
    console.log('Singleton is doing something!');
  }
}

// Usage
const singleton1 = Singleton.getInstance();
const singleton2 = Singleton.getInstance();

if (singleton1 === singleton2) {
  console.log('Singleton works, both variables contain the same instance.');
} else {
  console.log('Singleton failed, variables contain different instances.');
}

singleton1.someBusinessLogic();
