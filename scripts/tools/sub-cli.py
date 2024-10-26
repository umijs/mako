#!/usr/bin/env python3

import networkx as nx
from networkx.drawing.nx_agraph import write_dot, read_dot
import argparse
import subprocess

def find_reachable_nodes_and_subgraph(dot_file, target_nodes, n, output_dot_file):
    # 读取 .dot 文件，创建图对象
    G = read_dot(dot_file)

    # BFS 搜索，找到 n 跳内的所有节点
    reachable_nodes = set()
    edges = set()

    # BFS 队列初始化
    queue = [(node, 0) for node in target_nodes]

    while queue:
        current_node, current_depth = queue.pop(0)

        if current_depth > n:
            continue

        reachable_nodes.add(current_node)

        # 遍历当前节点的前驱节点
        for predecessor in G.predecessors(current_node):
            queue.append((predecessor, current_depth + 1))
            edges.add((predecessor, current_node))

    # 构建子图
    subgraph = G.subgraph(reachable_nodes).copy()

    # 将子图保存为 .dot 文件
    write_dot(subgraph, output_dot_file)
    print(f"Subgraph with reachable nodes saved to {output_dot_file}")

def select_target_nodes(G):
    # 创建一个 {label: node_id} 的映射
    label_to_node = {}

    for node in G.nodes(data=True):
        node_id = node[0]
        node_label = node[1].get('label', node_id)  # 如果没有 label，使用节点 ID
        label_to_node[node_label] = node_id

    # 使用 fzf 选择节点 label，允许多选
    process = subprocess.Popen(
        ['fzf', '--multi'], stdin=subprocess.PIPE, stdout=subprocess.PIPE
    )
    fzf_input = "\n".join(label_to_node.keys()).encode('utf-8')
    stdout, _ = process.communicate(input=fzf_input)

    # 解析选择的节点，fzf 返回多个节点的 label 时是用换行符分隔的
    selected_labels = stdout.decode('utf-8').strip().split('\n')

    # 将选中的 labels 映射为节点 ID
    selected_nodes = [label_to_node[label] for label in selected_labels if label in label_to_node]

    return selected_nodes

def main():
    # 设置命令行参数
    parser = argparse.ArgumentParser(description='从 Graphviz .dot 文件中生成子图')
    parser.add_argument('dot_file', type=str, help='输入的 .dot 文件路径')
    parser.add_argument('-n', '--hops', type=int, default=3, help='最大跳数')
    parser.add_argument('-o', '--output', type=str, default='subgraph_output.dot', help='输出的子图 .dot 文件')

    # 解析命令行参数
    args = parser.parse_args()

    # 读取图
    G = read_dot(args.dot_file)

    # 使用 fzf 选择目标节点
    target_nodes = select_target_nodes(G)

    # 执行子图查找
    if target_nodes:
        find_reachable_nodes_and_subgraph(args.dot_file, target_nodes, args.hops, args.output)

if __name__ == '__main__':
    main()
