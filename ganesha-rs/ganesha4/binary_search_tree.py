class Node:
    """A node in the binary search tree."""
    def __init__(self, key):
        self.key = key
        self.left = None
        self.right = None

class BinarySearchTree:
    """Simple BST implementation with insert, find, delete and inorder traversal."""
    def __init__(self):
        self.root = None

    # ----------------- Public API -----------------
    def insert(self, key):
        """Insert a new key into the BST."""
        self.root = self._insert_recursive(self.root, key)

    def find(self, key):
        """Return True if key exists in the tree, else False."""
        return self._find_recursive(self.root, key) is not None

    def delete(self, key):
        """Delete a key from the BST if it exists."""
        self.root = self._delete_recursive(self.root, key)

    def inorder_traversal(self):
        """Yield keys in ascending order."""
        yield from self._inorder_recursive(self.root)

    # ----------------- Internal helpers -----------------
    def _insert_recursive(self, node, key):
        if node is None:
            return Node(key)
        if key < node.key:
            node.left = self._insert_recursive(node.left, key)
        elif key > node.key:
            node.right = self._insert_recursive(node.right, key)
        # duplicate keys are ignored
        return node

    def _find_recursive(self, node, key):
        if node is None:
            return None
        if key == node.key:
            return node
        if key < node.key:
            return self._find_recursive(node.left, key)
        else:
            return self._find_recursive(node.right, key)

    def _delete_recursive(self, node, key):
        if node is None:
            return None

        if key < node.key:
            node.left = self._delete_recursive(node.left, key)
        elif key > node.key:
            node.right = self._delete_recursive(node.right, key)
        else:  # node to delete found
            # case 1: no children
            if node.left is None and node.right is None:
                return None
            # case 2: one child
            if node.left is None:
                return node.right
            if node.right is None:
                return node.left
            # case 3: two children – find inorder successor (smallest in right subtree)
            succ = self._min_value_node(node.right)
            node.key = succ.key
            node.right = self._delete_recursive(node.right, succ.key)

        return node

    def _min_value_node(self, node):
        current = node
        while current.left is not None:
            current = current.left
        return current

    def _inorder_recursive(self, node):
        if node is not None:
            yield from self._inorder_recursive(node.left)
            yield node.key
            yield from self._inorder_recursive(node.right)

# ----------------- Example usage -----------------
if __name__ == "__main__":
    bst = BinarySearchTree()
    for value in [50, 30, 70, 20, 40, 60, 80]:
        bst.insert(value)

    print("In-order traversal:", list(bst.inorder_traversal()))

    print("Find 40:", bst.find(40))
    print("Find 90:", bst.find(90))

    bst.delete(30)
    print("After deleting 30, in-order:", list(bst.inorder_traversal()))
