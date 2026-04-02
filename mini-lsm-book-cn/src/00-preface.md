<!--
  mini-lsm-book © 2022-2025 by Alex Chi Z is licensed under CC BY-NC-SA 4.0
-->

# 前言

![横幅](./mini-lsm-logo.png)

本课程教你如何在 Rust 中构建一个简单的 LSM-Tree 存储引擎。

## 什么是 LSM，为什么选择 LSM？

日志结构合并树是维护键值对的数据结构。这种数据结构广泛应用于分布式数据库系统，如 [TiDB](https://www.pingcap.com) 和 [CockroachDB](https://www.cockroachlabs.com)，作为它们的底层存储引擎。[RocksDB](http://rocksdb.org)，基于 [LevelDB](https://github.com/google/leveldb)，是 LSM-Tree 存储引擎的一个实现。它提供了许多键值访问功能，并在许多生产系统中使用。

一般来说，LSM 树是一种追加友好的数据结构。将 LSM 与其他键值数据结构（如 RB-Tree 和 B-Tree）进行比较更直观。对于 RB-Tree 和 B-Tree，所有数据操作都是原地进行的。也就是说，当你想更新键对应的值时，引擎会用新值覆盖其原始的内存或磁盘空间。但在 LSM 树中，所有写操作，即插入、更新、删除，都是延迟应用到存储中的。引擎将这些操作批量处理成 SST（排序字符串表）文件并将它们写入磁盘。一旦写入磁盘，引擎就不会直接修改它们。在一个称为合并的特殊后台任务中，引擎将合并这些文件以应用更新和删除。

这种架构设计使得 LSM 树易于使用。

1. 数据在持久存储上是不可变的。并发控制更直接。将后台任务（合并）卸载到远程服务器是可能的。直接从云原生存储系统（如 S3）存储和服务数据也是可行的。
2. 更改合并算法允许存储引擎在读放大、写放大和空间放大之间取得平衡。数据结构是多功能的，通过调整合并参数，我们可以针对不同的工作负载优化 LSM 结构。

本课程将教你如何在 Rust 编程语言中构建一个基于 LSM 树的存储引擎。

## 先决条件

* 你应该了解 Rust 编程语言的基础知识。阅读 [Rust 书](https://doc.rust-lang.org/book/) 就足够了。
* 你应该了解键值存储引擎的基本概念，即为什么我们需要复杂的设计来实现持久化。如果你以前没有数据库系统和存储系统的经验，你可以在 [PingCAP Talent Plan](https://github.com/pingcap/talent-plan/tree/master/courses/rust/projects/project-2) 中实现 Bitcask。
* 了解 LSM 树的基础知识不是必需的，但我们建议你阅读一些相关内容，例如 LevelDB 的整体思想。事先了解它们会让你熟悉可变和不可变内存表、SST、合并、WAL 等概念。

## 你期望从本课程中获得什么

完成本课程后，你应该深入理解基于 LSM 的存储系统如何工作，获得设计此类系统的实践经验，并将所学应用于你的学习和职业生涯中。你将理解此类存储系统中的设计权衡，并找到设计基于 LSM 的存储系统以满足工作负载要求/目标的最佳方法。这门非常深入的课程涵盖了现代存储系统（即 RocksDB）的所有基本实现细节和设计选择，基于作者在几个类似 LSM 的存储系统中的经验，你将能够直接将所学应用于工业界和学术界。

### 结构

本课程是一个包含多个部分（周）的广泛课程。每周有七章；你可以在 2 到 3 小时内完成每一章。每个部分的前六章将指导你构建一个可工作的系统，每周的最后一章将是一个*零食时间*章节，在你前六天构建的内容上实现一些简单的东西。每章都有必做任务、*检查你的理解*问题和额外任务。

### 测试

我们提供了一个完整的测试套件和一些 CLI 工具，供你验证你的解决方案是否正确。请注意，测试套件并不详尽，你的解决方案在通过所有测试用例后可能不是 100% 正确的。在实现系统的后续部分时，你可能需要修复早期的错误。我们建议你彻底思考你的实现，特别是在涉及多线程操作和竞争条件时。

### 解决方案

我们在 mini-lsm 主仓库中有一个解决方案，实现了课程中要求的所有功能。同时，我们还有一个 mini-lsm 解决方案检查点仓库，其中每个提交对应课程中的一章。

保持这样的检查点仓库与 mini-lsm 课程同步是具有挑战性的，因为每个错误修复或新功能都必须经过所有提交（或检查点）。因此，这个仓库可能不使用最新的起始代码或包含 mini-lsm 课程的最新功能。

**TL;DR: 我们不保证解决方案检查点仓库包含正确的解决方案、通过所有测试或具有正确的文档注释。** 对于正确的实现和实现所有内容后的解决方案，请查看主仓库中的解决方案。[https://github.com/skyzh/mini-lsm/tree/main/mini-lsm](https://github.com/skyzh/mini-lsm/tree/main/mini-lsm)。

如果你在课程的某个部分卡住了，或者不确定在哪里实现功能，你可以参考这个仓库寻求帮助。你可以比较提交之间的差异，以了解发生了什么变化。你可能需要在课程的不同章节中多次修改 mini-lsm 课程中的一些函数，你可以在这个仓库中了解每章具体需要实现什么。

你可以访问解决方案检查点仓库：[https://github.com/skyzh/mini-lsm-solution-checkpoint](https://github.com/skyzh/mini-lsm-solution-checkpoint)。

### 反馈

我们非常感激你的反馈。我们根据学生的反馈在 2024 年从头重写了整个课程。请分享你的学习经验，帮助我们不断改进课程。欢迎加入 [Discord 社区](https://skyzh.dev/join/discord) 并分享你的经验。

为什么我们重写它的长故事：该课程最初计划作为一般指导，学生从一个空目录开始，根据我们的规范实现他们想要的任何东西。我们只有最少的测试来检查行为是否正确。然而，原始课程过于开放，给学习体验带来了巨大障碍。由于学生事先没有整个系统的概览，而且说明模糊，有时他们很难理解为什么做出设计决策以及他们需要实现什么目标。课程的某些部分非常紧凑，不可能在一章内交付预期的内容。因此，我们完全重新设计了课程，以获得更简单的学习曲线和更清晰的学习目标。原始的一周课程现在分为两周（第一周关于存储格式，第二周关于深入合并），并增加了关于 MVCC 的额外部分。我们希望你觉得这门课程有趣，并对你的学习和职业生涯有所帮助。我们要感谢在 [Feedback after coding day 1](https://github.com/skyzh/mini-lsm/issues/11) 和 [Hello, when is the next update plan for the course?](https://github.com/skyzh/mini-lsm/issues/7) 中评论的每个人——你的反馈极大地帮助我们改进了课程。

### 许可证

本课程的源代码根据 Apache 2.0 许可，而本书根据 CC BY-NC-SA 4.0 许可。

### 这门课程会永远免费吗？

是的！现在公开可用的所有内容将永远免费，并接收终身更新和错误修复。同时，我们可能会提供付费的代码审查和办公时间服务。对于 DLC 部分（*后续内容*章节），截至 2024 年，我们没有完成它们的计划，也尚未决定它们是否会公开可用。

## 社区

你可以加入 skyzh 的 Discord 服务器，与 mini-lsm 社区一起学习。

[![加入 skyzh 的 Discord 服务器](discord-badge.svg)](https://skyzh.dev/join/discord)

## 开始

现在，你可以在 [Mini-LSM 课程概述](./00-overview.md) 中了解 LSM 结构的概述。

## 关于作者

截至撰写时（2024 年初），Chi 获得了卡内基梅隆大学的计算机科学硕士学位和上海交通大学的学士学位。他一直致力于各种数据库系统，包括 [TiKV][db1]、[AgateDB][db2]、[TerarkDB][db3]、[RisingWave][db4] 和 [Neon][db5]。自 2022 年以来，他担任 [CMU 的数据库系统课程](https://15445.courses.cs.cmu) 的助教三个学期，在 BusTub 教育系统中，他为课程添加了许多新功能和更多挑战（查看重新设计的 [查询执行](https://15445.courses.cs.cmu.edu/fall2022/project3/) 项目和超级挑战性的 [多版本并发控制](https://15445.courses.cs.cmu.edu/fall2023/project4/) 项目）。除了在 BusTub 教育系统上工作外，他还维护 [RisingLight](https://github.com/risinglightdb/risinglight) 教育数据库系统。Chi 对探索 Rust 编程语言如何适应数据库世界感兴趣。如果你也对那个主题感兴趣，请查看他之前关于构建向量化表达式框架的课程 [type-exercise-in-rust](https://github.com/skyzh/type-exercise-in-rust) 和关于构建向量数据库的课程 [write-you-a-vector-db](https://github.com/skyzh/write-you-a-vector-db)。

[db1]: https://github.com/tikv/tikv
[db2]: https://github.com/tikv/agatedb
[db3]: https://github.com/bytedance/terarkdb
[db4]: https://github.com/risingwavelabs/risingwave
[db5]: https://github.com/neondatabase/neon

{{#include copyright.md}}