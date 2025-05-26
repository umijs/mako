# 项目分析报告

## 核心模块

### 打包器核心模块
```
crates/bundler-core/         # 打包器核心实现
├── js/                      # JavaScript相关代码
│   ├── src/react-refresh/   # React热更新运行时
├── src/                     # Rust源代码
│   ├── client/              # 客户端相关功能
│   │   ├── react_refresh.rs # React热更新实现
│   │   ├── transforms.rs    # 代码转换功能
│   ├── config.rs            # 配置处理
│   ├── embed_js.rs          # JavaScript嵌入功能
│   ├── library/             # 打包库相关
│   │   ├── chunking_context.rs # 代码分割上下文
│   │   ├── ecmascript/      # ECMAScript相关
│   ├── server/              # 服务端功能
│   ├── shared/              # 共享功能
│   │   ├── transforms/      # 各种代码转换器
│   │   │   ├── emotion.rs   # Emotion CSS转换
│   │   │   ├── styled_jsx.rs # styled-jsx转换
├── Cargo.toml               # Rust项目配置
```

### 打包器API
```
crates/bundler-api/          # 打包器API接口
├── src/
│   ├── endpoints.rs         # API端点定义
│   ├── entrypoints.rs       # 入口点处理
│   ├── esm.rs               # ES模块处理
│   ├── hmr.rs               # 热模块替换实现
│   ├── project.rs           # 项目管理
│   ├── utils.rs             # 工具函数
├── Cargo.toml
```

### 命令行工具
```
crates/cli/                  # 命令行工具
├── src/
│   ├── bin/                 # 命令行入口
│   │   ├── utoo-*.rs        # 各种子命令实现
│   ├── cmd/                 # 命令实现
│   ├── helper/              # 辅助功能
│   │   ├── auto_update.rs   # 自动更新
│   │   ├── package.rs       # 包管理
│   ├── service/             # 服务功能
│   │   ├── install.rs       # 安装服务
│   │   ├── update.rs        # 更新服务
│   ├── util/                # 工具类
│   │   ├── cache.rs         # 缓存处理
│   │   ├── downloader.rs    # 下载器
├── Cargo.toml
```

## NAPI模块
```
crates/bundler-napi/         # Node-API绑定
├── src/
│   ├── bundler_api/         # API绑定
│   │   ├── endpoint.rs      # 端点绑定
│   │   ├── project.rs       # 项目绑定
├── Cargo.toml
```

## 配置模块
```
.cargo/                      # Cargo配置
├── config.toml              # 构建配置
.github/                     # GitHub配置
├── workflows/               # CI/CD工作流
│   ├── bench.yml            # 基准测试工作流
│   ├── ci.yml               # 持续集成
│   ├── release.yml          # 发布工作流
Cargo.toml                   # 根项目配置
package.json                 # NPM项目配置
rust-toolchain.toml          # Rust工具链配置
```

## 示例模块
```
examples/                    # 使用示例
├── with-antd/               # Ant Design示例
│   ├── src/pages/           # 示例页面
│   │   ├── ant-design-pro.tsx # Ant Design Pro演示
├── with-less/               # LESS示例
├── with-sass/               # Sass示例
├── with-style-loader/       # 样式加载器示例
```

## 打包发布模块
```
packages/                    # NPM发布包
├── bundler/                 # 主打包器
│   ├── npm/                 # 多平台二进制发布
│   │   ├── darwin-arm64/    # Mac M1平台
│   │   ├── linux-x64-gnu/   # Linux x64
│   ├── src/                 # TypeScript源代码
│   │   ├── project.ts       # 项目类型定义
├── loader-runner/           # 加载器运行器
├── style-loader/            # 样式加载器
```

## 辅助工具
```
vendor/                      # 发布辅助工具
├── scripts/                 # 脚本
│   ├── npm-*.sh             # 发布脚本
├── templates/               # 模板文件
│   ├── *.template           # 各种package.json模板
```

## 测试与验证
```
crates/cli/benches/          # 性能基准测试
│   └── deps_benchmark.rs    # 依赖解析基准测试
```