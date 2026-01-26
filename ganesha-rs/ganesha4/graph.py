"""
Graph Data Structure with BFS and DFS implementations.

Author: Ganesha
Date: 2026-01-25
"""

from collections import defaultdict, deque

class Graph:
    def __init__(self):
        # adjacency list representation
        self.adj = defaultdict(list)

    def add_edge(self, src, dest, bidirectional=True):
        """Add an edge from src to dest.
        If bidirectional is True (default), also add reverse edge."""
        self.adj[src].append(dest)
        if bidirectional:
            self.adj[dest].append(src)

    def bfs(self, start):
        """
        Breadth-First Search traversal starting from 'start'.
        Returns a list of nodes in the order they are visited.
        """
        visited = set()
        queue = deque([start])
        order = []

        while queue:
            node = queue.popleft()
            if node not in visited:
                visited.add(node)
                order.append(node)
                for neighbor in self.adj[node]:
                    if neighbor not in visited:
                        queue.append(neighbor)
        return order

    def dfs(self, start):
        """
        Depth-First Search traversal starting from 'start'.
        Returns a list of nodes in the order they are visited.
        Uses an explicit stack to avoid recursion limits.
        """
        visited = set()
        stack = [start]
        order = []

        while stack:
            node = stack.pop()
            if node not in visited:
                visited.add(node)
                order.append(node)
                # Add neighbors in reverse order so that the first neighbor
                # is processed first (mimics recursive DFS).
                for neighbor in reversed(self.adj[node]):
                    if neighbor not in visited:
                        stack.append(neighbor)
        return order

    def __str__(self):
        """String representation of the adjacency list."""
        return '\n'.join(f"{node}: {neighbors}" for node, neighbors in self.adj.items())

# Example usage
if __name__ == "__main__":
    g = Graph()
    edges = [
        (1, 2), (1, 3), (2, 4),
        (2, 5), (3, 6), (5, 6)
    ]
    for u, v in edges:
        g.add_edge(u, v)

    print("Graph adjacency list:")
    print(g)

    print("\nBFS traversal starting from node 1:")
    print(g.bfs(1))

    print("\nDFS traversal starting from node 1:")
    print(g.dfs(1))
