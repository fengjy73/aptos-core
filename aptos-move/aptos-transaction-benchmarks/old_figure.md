
# 0) 统一记号与事件模型（与你的插桩对齐）

* 一个区块包含 $N=100{,}000$ 笔交易，按**预设顺序**编号为 $\{1,2,\dots,N\}$。
* 线程数（可用核数）记为 $\theta\in\{1,2,\dots,256\}$。
* 交易 $j$ 的**第 $m$ 次执行**叫一次 **incarnation**，记为 $(j,m)$。
* 时间轴连续，区块开始/结束时间分别为 $t_0,t_1$。
* 三个核心**活跃集合**（随时间变化）：

  * $E(t)$：正在 **EXECUTING** 的 $(j,m)$；
  * $S(t)$：处于 **SUSPENDED(挂起等待)** 的 $(j,m)$；
  * $V(t)$：处于 **VALIDATING** 的 $(j,m)$。
* **活跃执行宽度**：$W(t)=|E(t)|$。
* **采样**：把时间离散为等间隔 $\Delta$（建议 1–2ms），采样点集合 $\mathcal{T}=\{t_0,t_0+\Delta,\dots,t_1\}$。
* **调度下界索引**（Block-STM 两个原子计数器）：

  * $\mathrm{exec\_idx}(t)$：最小可执行下标；
  * $\mathrm{val\_idx}(t)$：最小可验证下标。
* **依赖图**：当 $(j,m)$ 在执行中读到来自交易 $i$ 的 **ESTIMATE** 值而**暂停**时，记一条有向边 $i\to j$（一次暂停就是一次边事件）。
* **读写键**（FA 模型）：对每笔 `from,to,value`，读写键集合近似为
  $\mathsf{keys}(j)=\{\,\texttt{FungibleStore}(\texttt{from}_j),\ \texttt{FungibleStore}(\texttt{to}_j)\,\}.$

> 事件日志最小集：`exec_begin/end, read(..., source∈{storage,mv,estimate}), suspend(depends_on=i), validate_begin/end(result), val_fail_detail(key,violator=i), mark_estimate, schedule_revalidation(affected_count), dep_edge_{create,resolve}, sched_sample(exec_idx,val_idx,|E|,|V|,active,done_marker)`。

---

# 1) 系统与并行度指标（图 F0, F1, F2, F3, F12）

**吞吐(TPS)**

$$
\mathrm{TPS}(\theta)=\frac{N}{t_1-t_0}\quad[\text{tx/s}]
$$

**执行并行宽度（瞬时）**

$$
W(t)=|E(t)|
$$

**并行利用率（时间分位归一化）**

$$
\eta_p(\theta)=\operatorname{median}_{t\in\mathcal{T}}\!\left(\frac{\min\{W(t),\theta\}}{\theta}\right),
\quad p\ \text{用于强调“按时间中位”口径}
$$

**状态占比（随时间）**

$$
\phi_X(t)=\frac{|X(t)|}{|E(t)|+|S(t)|+|V(t)|}\ ,\quad X\in\{E,S,V\}
$$

**阶段时长分解（每次 incarnation）**
设 $(j,m)$ 的执行开始/结束为 $t^{\mathrm{ex}}_{j,m,\mathrm{beg}},t^{\mathrm{ex}}_{j,m,\mathrm{end}}$；
应用写集为 $[t^{\mathrm{ap}}_{j,m,\mathrm{beg}},t^{\mathrm{ap}}_{j,m,\mathrm{end}}]$；
验证为 $[t^{\mathrm{va}}_{j,m,\mathrm{beg}},t^{\mathrm{va}}_{j,m,\mathrm{end}}]$；
第 $u$ 次暂停窗口为 $[t^{\mathrm{su}}_{j,m,u,\mathrm{beg}},t^{\mathrm{su}}_{j,m,u,\mathrm{end}}]$。则

$$
\begin{aligned}
T^{\mathrm{exec}}_{j,m}&=t^{\mathrm{ex}}_{j,m,\mathrm{end}}-t^{\mathrm{ex}}_{j,m,\mathrm{beg}},\\
T^{\mathrm{apply}}_{j,m}&=t^{\mathrm{ap}}_{j,m,\mathrm{end}}-t^{\mathrm{ap}}_{j,m,\mathrm{beg}},\\
T^{\mathrm{val}}_{j,m}&=t^{\mathrm{va}}_{j,m,\mathrm{end}}-t^{\mathrm{va}}_{j,m,\mathrm{beg}},\\
T^{\mathrm{susp}}_{j,m}&=\sum_{u}\left(t^{\mathrm{su}}_{j,m,u,\mathrm{end}}-t^{\mathrm{su}}_{j,m,u,\mathrm{beg}}\right).
\end{aligned}
$$

**提交门槛“尾巴”**（double-collect gate）
设区块内**最后一个执行结束**时间为

$$
t^{\mathrm{last\_exec}}=\max_{j,m:\ \text{最终生效的 incarnation}} t^{\mathrm{ex}}_{j,m,\mathrm{end}},
$$

**完成标记**（两下界到顶且无在途任务，经双重收集确认）的时间为 $t^{\mathrm{done}}$，则

$$
\mathrm{Tail}(\theta)=t^{\mathrm{done}}-t^{\mathrm{last\_exec}}.
$$

---

# 2) OCC/回滚/重试与验证风暴（图 F3, F4, F11）

**重试次数（每交易）**
若交易 $j$ 共经历 $n_j$ 次 incarnation，

$$
r_j=n_j-1.
$$

**回滚率（区块级）**
设 $\mathcal{A}$ 为**因验证失败**或**被迫重执行**的 incarnation 集合（不含最终成功那一次），

$$
\mathrm{AbortRate}=\frac{|\mathcal{A}|}{\sum_{j} n_j }.
$$

**再验证波触发规模（由失败交易 $i$ 触发）**
当 $i$ 验证失败后，框架为所有 $j>i$ 安排再验证；设该波的被安排集合为 $\mathcal{R}_i$，其中真正再次**检测出冲突**的集合为 $\mathcal{R}_i^{\mathrm{eff}}$，则

$$
\begin{aligned}
\mathrm{RF\_all}(i)&=|\mathcal{R}_i|,\\
\mathrm{RF\_eff}(i)&=|\mathcal{R}_i^{\mathrm{eff}}|,\\
\mathrm{Redundancy}(i)&=1-\frac{\mathrm{RF\_eff}(i)}{\mathrm{RF\_all}(i)}.
\end{aligned}
$$

---

# 3) 暂停传播/依赖图指标（图 F5, F6）

令 **暂停依赖图**为 $G=(V,E)$，顶点 $V=\{1,\dots,N\}$，存在边 $i\to j$ 当且仅当 $j$ 在执行中**读到 $i$ 的 ESTIMATE** 而发生暂停。每个暂停事件都有等待时长 $\Delta t_{j}^{(i)}$（由 `dep_edge_{create,resolve}` 获得）。

**暂停扇出（每个源 $i$）**

$$
\mathrm{Outdeg}(i)=|\{\,j\mid (i\to j)\in E\,\}|.
$$

**暂停深度（图的最长路径）**

$$
\mathrm{Depth}(G)=\max_{j}\ \max_{\text{路径 }i_0\to i_1\to\cdots\to i_k=j}\ k.
$$

**暂停影响分数（SIS, 归因到源 $i$）**
（把**所有直接/间接后继**的等待时间累加到源）

$$
\mathrm{SIS}(i)=\sum_{j:\ i\leadsto j}\ \Delta t_{j}^{(\mathrm{parent}(j))}
$$

其中 $i\leadsto j$ 表示图中从 $i$ 到 $j$ 有路径，且每个暂停事件按其直接父节点 `depends_on_tx` 计一次（不会重复归因同一事件给多父）。

---

# 4) 推测有效/浪费归因（图 F3, F13, F14）

**推测有效比**（全区块）
设 $\mathcal{S}$ 为**最终成功**的 incarnation 集合，$\mathcal{E}$ 为**所有执行**的 incarnation 集合，则

$$
\mathrm{SpecEff}=\frac{\sum_{(j,m)\in\mathcal{S}} T^{\mathrm{exec}}_{j,m}}{\sum_{(j,m)\in\mathcal{E}} T^{\mathrm{exec}}_{j,m}}.
$$

**推测浪费归因到前序“违规者” SWA**
当 $j$ 的某次 $(j,m)$ 因验证失败且 `val_fail_detail.violator_tx=i`，将该次**执行时长**归因给 $i$：

$$
\mathrm{SWA}(i)=\sum_{\substack{(j,m)\ \text{失败}\\ \mathrm{violator}(j,m)=i}}\ T^{\mathrm{exec}}_{j,m}.
$$

**提前中止节省（Saved Waste，选做）**
若 $(j,m)$ 在执行中 **读到 ESTIMATE** 被暂停，其时刻到“若不暂停的预计完成时刻”的差值可近似为潜在节省，记

$$
\mathrm{Saved}(j,m)=\widehat{t^{\mathrm{ex}}_{j,m,\mathrm{end}}}-t^{\mathrm{su}}_{j,m,\mathrm{beg}},
$$

区块级合计

$$
\mathrm{SavedTotal}=\sum_{j,m}\mathrm{Saved}(j,m).
$$

（$\widehat{t^{\mathrm{ex}}_{j,m,\mathrm{end}}}$ 可用该交易以往 incarnation 的执行时间中位数估计。）

---

# 5) 估计（ESTIMATE）与多版本（图 F7, F8, F9）

**估计命中率（按交易聚合）**
设 $\mathsf{EstKeys}_{j,m}$ 是 $j$ 在一次失败后被标记为 ESTIMATE 的键集；下一次成功的写集为 $\mathsf{Write}_{j,m+1}$，则

$$
\mathrm{Hit}(j)=\frac{\left|\bigcup_{m} \left(\mathsf{EstKeys}_{j,m}\cap \mathsf{Write}_{j,m+1}\right)\right|}{\left|\bigcup_{m}\mathsf{EstKeys}_{j,m}\right|}.
$$

**新键写入比例（跨 incarnation）**
设 $\mathsf{Write}^{\star}_{j,m}$ 为 $(j,m)$ 的写集，$\mathsf{Write}^{\star}_{j,m-1}$ 为**上一次成功** incarnation 的写集，则

$$
\mathrm{EMR}(j)=\frac{\sum_{m>1}\left|\mathsf{Write}^{\star}_{j,m}\setminus \mathsf{Write}^{\star}_{j,m-1}\right|}{\sum_{m>1}\left|\mathsf{Write}^{\star}_{j,m}\right|}.
$$

**读源占比（按时间/按交易）**
在采样集合 $\mathcal{T}$ 上统计

$$
\begin{aligned}
\pi_{\mathrm{storage}}&=\frac{\#\text{reads from storage}}{\#\text{all reads}},\\
\pi_{\mathrm{mv}}&=\frac{\#\text{reads from committed MV}}{\#\text{all reads}},\\
\pi_{\mathrm{est}}&=\frac{\#\text{reads of ESTIMATE}}{\#\text{all reads}},\quad
\pi_{\mathrm{storage}}+\pi_{\mathrm{mv}}+\pi_{\mathrm{est}}=1.
\end{aligned}
$$

**版本链长度（每键）**
对每个键 $k$，统计区块内创建的版本数量（含 ESTIMATE/最终版，按实现口径一致）：

$$
L_k=\#\{\text{versions of }k\ \text{in the block}\}.
$$

**版本链贡献（每交易）**

$$
\mathrm{VCC}(j)=\sum_{k\in \mathsf{Write}^{\text{commit}}_{j}} 1,
$$

（若要计入失败产生的临时版本，可并列报告 $\mathrm{VCC}^{\text{all}}(j)$）。

---

# 6) 调度公平/低序优先/优先反转（图 F10）

**最低待处理索引**

$$
m(t)=\min\{\,x\in\{1,\dots,N\}\mid x\ \text{尚未完成最终 incarnation}\,\}.
$$

**优先反转事件指示**（对线程 $q$ 在时刻 $t$ 正执行索引为 $j_q(t)$ 的任务）

$$
I_q(t)=\mathbf{1}\{\,j_q(t)>m(t)\,\}.
$$

**优先反转占比（区块级）**

$$
\mathrm{PII}=\frac{1}{\theta\,(t_1-t_0)}\sum_{q=1}^{\theta}\int_{t_0}^{t_1} I_q(t)\,dt.
$$

**执行任务供给不足（E-starved）占比**

$$
\mathrm{LID}=\frac{1}{t_1-t_0}\int_{t_0}^{t_1}\mathbf{1}\{\,|E(t)|<\theta\,\}\,dt.
$$

---

# 7) “并发杀手”综合评分（图 F13, F22）

对每个交易 $i$ 定义综合贡献（可用于排序）：

$$
\mathrm{KillerScore}(i)=\alpha\,\mathrm{SIS}(i)+\beta\,\mathrm{SWA}(i)+\gamma\,\mathrm{RF\_all}(i),
$$

其中 $\alpha,\beta,\gamma>0$ 为可调权重（默认 $\alpha=\beta=1,\ \gamma=0.1$ 以免 RF 量纲压倒）。

> 也可把指标聚合到**地址/键**：把所有写入该键或涉及该地址的 $i$ 的得分求和，得到“并发杀手地址榜”。

---

# 8) ETH vs USDT 负载画像与行为差异（图 F17–F21）

**地址度（出现频次）**
对地址 $a$，

$$
d(a)=\#\{\,j\mid \texttt{from}_j=a\ \text{或}\ \texttt{to}_j=a\,\}.
$$

**Top-K 覆盖曲线**

$$
C(K)=\frac{\sum_{a\in \mathrm{TopK}(d)} d(a)}{\sum_{a} d(a)}.
$$

**Gini 系数**（按地址度分布）
设地址度按升序 $x_1\le \cdots\le x_M$，

$$
\mathrm{Gini}=1-\frac{2}{M-1}\left(M-\frac{\sum_{i=1}^{M}(M+1-i)\,x_i}{\sum_{i=1}^{M}x_i}\right).
$$

**键重合率（同窗口内）**
把区块按顺序划分为窗口集合 $\mathcal{W}$（如每 1000 笔一窗），对窗口 $w$：

$$
\mathrm{Overlap}(w)=\frac{\#\{(j_1,j_2)\in w:\ \mathsf{keys}(j_1)\cap \mathsf{keys}(j_2)\neq\emptyset\}}{\binom{|w|}{2}},
$$

全区块取均值或分布即可。

> 同样的定义套在 ETH 与 USDT 两个数据集上，用以解释后续**并行宽度饱和**、**重试加重**、**暂停更深**等差异。

---

# 9) 图表规范（坐标、图型、如何从日志算、预期现象）

下面以编号快速给出**每张图的要点**（都能按上面公式直接算出来）。

### F0 扩核曲线：吞吐 & 并行利用率

* **类型**：双轴折线。
* **X**：线程数 $\theta$（对数刻度可选）。
* **Y1**：$\mathrm{TPS}(\theta)$；**Y2**：$\eta_p(\theta)$。
* **预期**：两条曲线在某 $\theta^\star$ 附近出现拐点/平台。
* **分析**：标注 $\theta^\star$；在该段取样交叉到 F1、F11、F13 看根因。

### F1 状态时序 + 下界推进

* **类型**：上堆叠面积（$\phi_E,\phi_S,\phi_V$），下双折线（$\mathrm{exec\_idx},\mathrm{val\_idx}$）。
* **X**：时间（ms）。
* **预期**：**验证风暴**时 $\phi_V$ 峰值、$\mathrm{val\_idx}$ 迅速推进。
* **分析**：截出峰值窗口，转到 F11、F4。

### F2 并行宽度分布

* **类型**：箱线/小提琴。
* **X**：$\theta$；**Y**：样本化 $W(t)$ 的分位（p50/p95）。
* **预期**：随 $\theta$ 增长的**边际提升递减**，在 $\theta^\star$ 后饱和。
* **分析**：说明**软件可并行度上界**。

### F3 时间分解

* **类型**：堆叠条形或 ECDF。
* **Y**：$T^{\mathrm{exec}},T^{\mathrm{apply}},T^{\mathrm{val}},T^{\mathrm{susp}}$。
* **预期**：高冲突时 $T^{\mathrm{val}}$ 与 $T^{\mathrm{susp}}$ 占比上升。
* **分析**：把等待（暂停）与验证区分开来，为 F6/F11 做铺垫。

### F4 重试成本

* **类型**：ECDF（$r_j$）；折线（AbortRate vs $\theta$）。
* **预期**：USDT/高冲突下尾更厚，AbortRate 更高。
* **分析**：与 F2 的饱和点一致性。

### F5 失败因果

* **类型**：气泡（x=键/地址，y=失败次数，泡=牵涉写者数）；直方（$\mathrm{RF\_all}(i)$）。
* **预期**：极少数地址占主要失败。
* **分析**：把这些键记入“热点名单”，在 F22 汇总。

### F6 暂停传播

* **类型**：直方或箱线（$\mathrm{Outdeg}(i)$、$\mathrm{Depth}(G)$）。
* **预期**：USDT/高冲突时深度与扇出显著上升。
* **分析**：对应 F3 的 $T^{\mathrm{susp}}$ 增幅。

### F7 估计有效性

* **类型**：折线（$\mathrm{Hit}(j)$ 的均值/分位 vs $\theta$）；箱线（$\mathrm{EMR}(j)$）。
* **预期**：若 $\mathrm{Hit}$ 低且 $\mathrm{EMR}$ 高 → **冗余再验证**。
* **分析**：与 F11 的冗余率对齐。

### F8 读源占比

* **类型**：堆叠柱（$\pi_{\mathrm{storage}},\pi_{\mathrm{mv}},\pi_{\mathrm{est}}$）。
* **预期**：冲突增 → $\pi_{\mathrm{est}}$ 升，$\pi_{\mathrm{mv}}$ 降。
* **分析**：解释并行度下降的**一手证据**。

### F9 版本链

* **类型**：直方（$L_k$），小图时序（Top-K 键的 $L_k$ 随时间）。
* **预期**：热点键形成长链。
* **分析**：与 F5/F6 的热点一致。

### F10 调度兑现度

* **类型**：热力图（线程×时间显示 $j_q(t)$ 的相对位置）；折线（$\mathrm{exec\_idx},\mathrm{val\_idx}$）。
* **指标**：$\mathrm{PII},\ \mathrm{LID}$。
* **预期**：健康期 $\mathrm{PII}$ 低、$\mathrm{LID}$ 低；V-storm 期反之。
* **分析**：如 $\mathrm{PII}$ 偏高，考虑调度策略与验证触发阈值。

### F11 再验证波

* **类型**：热力矩阵（行触发者 $i$，列受影响 $j>i$，色=是否再次冲突/耗时）。
* **指标**：$\mathrm{RF\_all}(i),\ \mathrm{RF\_eff}(i),\ \mathrm{Redundancy}(i)$。
* **预期**：大片“浅色”＝冗余率高。
* **分析**：与 F7 的 $\mathrm{Hit},\mathrm{EMR}$ 联动。

### F12 提交尾巴

* **类型**：箱线（$\mathrm{Tail}(\theta)$）。
* **预期**：末端少数交易拖尾，尾延迟在高冲突/USDT 更长。
* **分析**：与 F5/F6 的末端热点对应。

### F13 并发杀手帕累托

* **类型**：条形（按 $\mathrm{KillerScore}(i)$ 排序 Top-N）。
* **预期**：前 1–5% 交易贡献 60–90% 的损失。
* **分析**：定点剖析前几名（做 F14 瀑布）。

### F14 暂停瀑布（单个元凶交易的扇出时间图）

* **类型**：瀑布/甘特（受害者 $j$ 在纵轴，横轴是等待窗口）。
* **预期**：显示“单点引起的大面积停顿”的时间结构。
* **分析**：结合 $\mathrm{SIS}(i)$、其键 $k$、版本链 $L_k$ 给质性解释。

### F17 Top-K 覆盖（ETH vs USDT）

* **类型**：折线（$C(K)$，对数 X）。
* **预期**：USDT 更陡（集中度更高）。
* **分析**：为后续行为差异提供负载依据。

### F18 并行宽度对照

* **类型**：折线两条（ETH/USDT），Y 为 p50/p95 的 $W(t)$。
* **预期**：USDT 更早饱和。
* **分析**：连到 F19–F21 的差异链。

### F19 重试 ECDF 对照

* **类型**：ECDF（ETH/USDT）。
* **预期**：USDT 尾更厚。
* **分析**：说明更高冲突/更强热点。

### F20 暂停深度/扇出对照

* **类型**：箱线（ETH/USDT）。
* **预期**：USDT 显著更高（可做 Mann–Whitney/KS）。
* **分析**：统计显著性标注 \* 或 \*\*。

### F21 冗余再验证率对照

* **类型**：折线（Y 为 $\mathrm{Redundancy}$，X 为 $\theta$）。
* **预期**：USDT 更高。
* **分析**：与 F7 的 $\mathrm{Hit},\mathrm{EMR}$ 共同解释。

### F22 并发杀手对照（交易或地址）

* **类型**：并排条形（ETH vs USDT Top-N）。
* **预期**：USDT 的头部集中更强。
* **分析**：点名前 5 地址，关联 F5/F9 的热点键。

---

# 10) ETH 与 USDT 在 FA 上的映射提醒（确保一一对应）

* 映射键：$\mathsf{keys}(j)=\{\texttt{FungibleStore}(\texttt{from}_j),\texttt{FungibleStore}(\texttt{to}_j)\}$。
* 没有 `allowance/transferFrom`，因此冲突主要来自**地址重合**与热点。
* 为避免“地址字符串”带来不必要开销，先做**地址 ID 压缩**，确保日志键是整型 ID。

---

# 11) 最后落地建议

* 统一采样步长 $\Delta=1\ \text{ms}$；每个 $\theta$ 跑 ≥3 个 100k 区块，画均值±95%CI。
* Key-Access 日志可**1–5% 抽样**，上述指标照样能算（F8/F9 用抽样估计）。
* 先把 **F0,F1,F2,F3,F5,F11** 跑出来，就能定位“拐点区间”的**并发杀手**；再做 **F13/F14** 给强证据。
* 对比 ETH/USDT，最先上 **F17–F19** 三张就能把差异讲“明白”。

---
