# 大作业：在线评测系统

2022 年夏季学期《程序设计训练》 Rust 课堂大作业 (二)。

## 程序结构

```c
src
├── api
│   ├── contests.rs     // 比赛相关 API
│   ├── err.rs          // 统一的错误接口
│   ├── jobs.rs         // 评测任务相关 API
│   └── users.rs        // 用户相关 API
├── api.rs              // API 模块
├── authorization.rs    // 用户鉴权及相关 API
├── config.rs           // 配置的读入及解析
├── judge.rs            // 独立评测进程
├── main.rs             // 服务器主函数
├── persistent          // 持久化模块
│   ├── models          // 持久化数据模型及相关函数
│   │   ├── contests.rs // 比赛相关模型及函数
│   │   ├── jobs.rs     // 评测任务相关模型及函数
│   │   └── users.rs    // 用户相关模型及函数
│   ├── models.rs       // 数据模型模块
│   └── schema.rs       // 模型到数据库表的映射关系
└── persistent.rs       // 持久化模块
```

此外，整个 package 还提供了一个 feature flag `authorization` 来控制用户鉴权功能的开启与否。

## OJ 主要功能说明

### 配置

在 `config.json` 中可以对 OJ 进行配置，包括服务器绑定的地址、端口以及题目配置信息。

```json
{
  "server": {
    "bind_address": "127.0.0.1",				// 绑定地址
    "bind_port": 12345							// 绑定端口
  },
  "problems": [
    {
      "id": 0,									// 题目 ID
      "name": "aplusb",							// 题目名称
      "type": "standard",						// 题目类型，支持 standard 与 strict (严格比较)
      "misc": {},								// 附加信息
      "cases": [								// 测试点
        {
          "score": 50.0,						// 分数
          "input_file": "./data/aplusb/1.in",	// 输入文件
          "answer_file": "./data/aplusb/1.ans",	// 输出文件
          "time_limit": 1000000,				// 时间限制 (单位为微秒)，0 表示不限制
          "memory_limit": 1048576				// 内存限制 (单位为字节)
        }, {
          "score": 50.0,
          "input_file": "./data/aplusb/2.in",
          "answer_file": "./data/aplusb/2.ans",
          "time_limit": 1000000,
          "memory_limit": 1048576
        }
      ]
    }
  ],
  "languages": [								// 支持的语言配置
    {
      "name": "Rust",							// 语言名称
      "file_name": "main.rs",					// 语言源代码文件名
      // 编译命令，其中 %INPUT% 与 %OUTPUT% 为源代码文件与可执行文件的占位符
      "command": ["rustc", "-C", "opt-level=2", "-o", "%OUTPUT%", "%INPUT%"]
    }
  ]
}
```

运行时，必须指定命令行参数 `--config <PATH>` 来指明配置文件路径，可以指定 `--flush-data` 来清除保存的持久化数据。

### 权限

已登录用户的权限等级分为 `User` (普通用户)、`Author` (出题人) 以及 `Admin` (管理员) 三级。较高权限等级能够访问所有更低权限等级能访问的 API，因此下方仅标注访问 API 需要的最低权限等级。

若尝试访问无权访问的 API，会得到 `ERR_FORBIDDEN` 错误。

### 错误

在请求错误时，会返回错误响应，格式如下：

```json
{
    "code": ...,
    "reason": "...",
    "message": "..."
}
```

其中各种错误对应的状态码、错误代码、原因如下：

| 错误代码 | 状态码                      | `reason`               | 含义                       |
| -------- | --------------------------- | ---------------------- | -------------------------- |
| 1        | `400 Bad Request`           | `ERR_INVALID_ARGUMENT` | 提供的参数存在错误         |
| 2        | `400 Bad Request`           | `ERR_INVALID_STATE`    | 对象目前状态无法进行此操作 |
| 3        | `404 Not Found`             | `ERR_NOT_FOUND`        | 某个对象不存在             |
| 4        | `400 Bad Request`           | `ERR_RATE_LIMIT`       | 提交次数过多               |
| 5        | `500 Internal Server Error` | `ERR_EXTERNAL`         | 外部错误                   |
| 6        | `500 Internal Server Error` | `ERR_INTERNAL`         | 内部错误                   |
| 7        | `403 Forbidden`             | `ERR_FORBIDDEN`        | 无权进行此操作             |

若提供的请求格式不正确（例如在期望 JSON 请求体的 API 提供了错误的 JSON 格式请求体），则会得到 `ERR_INVALID_ARGUMENT` 错误。此外，在鉴权模式下，进行除了 `POST /register` 与 `POST /login` 此外的操作会得到 `401 Unauthorized` 响应。

### 用户管理

#### POST /register

**需求权限：**任何人均可访问（包括未登录用户）

使用此 API 注册一个用户，请求格式如下：

```json
{
    "username": "USERNAME",
    "password": "PASSWORD"
}
```

注册后的用户权限等级为 `User`。若用户名已被占用将得到 `ERR_INVALID_ARGUMENT` 错误。若请求正确，则响应体如下：

```json
{
    "id": id,
    "name": "name"
}
```

其中 `id` 为新创建用户的 ID。

#### POST /login

**需求权限：**任何人均可访问（包括未登录用户）

使用此 API 进行登录，请求格式如下：

```json
{
    "username": "USERNAME",
    "password": "PASSWORD"
}
```

登录成功后会得到一个会话生存期的 Cookie，此后在此会话内无需再次登陆。若请求正确，则响应体如下：

```json
{
    "id": id,
    "name": "name"
}
```

其中 `id` 为登录用户的 ID。

#### POST /passwd

**需求权限：**`User`

使用此 API 更改自己的密码，请求格式如下：

```json
{
    "old_password": "passwd",
    "new_password": "new_passwd"
}
```

若密码错误，将得到 `ERR_INVALID_ARGUMENT` 错误。若请求正确，响应体为空。

#### POST /privilege

**需求权限：**`Admin`

使用此 API 更改用户权限。请求格式如下：

```json
{
    "username": "username",
    "role": "Role"
}
```

若用户不存在将得到 `ERR_NOT_FOUND` 错误。若请求正确，响应体为空。

#### POST /users

**需求权限：**`Admin`

使用此 API 创建或更新用户并指定密码和权限。请求格式如下：

```json
{
    "id": 12345,
    "name": "998244353",
    "password": "353442899",
    "role": "Author"
}
```

若提供了 `id` 字段，则更新指定用户，否则创建新用户。新建用户时，若不提供 `role` 字段，则默认权限为 `User`。更新用户时未提供的字段不会被改变。

若提供了 ID 但找不到指定用户，则返回 `ERR_NOT_FOUND` 错误。若用户名已被占用，则返回 `ERR_INVALID_ARGUMENT` 错误。

若请求正确，则响应体如下：

```json
{
    "id": id,
    "name": "name"
}
```

其中 `id` 为新建或更新用户的 ID。

#### GET /users

**需求权限：**`User`

使用此 API 获取用户列表。不需要参数，返回格式为一个数组，其中每个对象的格式与 `POST /users` 相同。

### 评测

#### POST /jobs

**需求权限：**`User`

向 OJ 发送 `POST /jobs` 请求，即可发起一个评测请求。请求体为 JSON 格式，格式如下：

```json
{
  "source_code": "fn main() { println!(\"Hello, world!\"); }",	// 源代码
  "language": "Rust",											// 语言
  "user_id": 0,													// 用户 ID
  "contest_id": 0,												// 比赛 ID
  "problem_id": 0												// 题目 ID
}
```

请求会将评测任务加入评测队列后立刻返回，若无参数错误，返回格式如下：

```json
{
  "id": 0,														// 评测任务 ID
  "created_time": "2022-08-27T02:05:29.000Z",					// 评测任务创建时间
  "updated_time": "2022-08-27T02:05:30.000Z",					// 评测任务更新时间
  "submission": {												// 提交信息
    "source_code": "fn main() { println!('Hello World!'); }",
    "language": "Rust",
    "user_id": 0,
    "contest_id": 0,
    "problem_id": 0
  },
  "state": "Queuing",											// 评测任务状态
  "result": "Waiting",											// 评测结果
  "score": 87.5,												// 得分
  "cases": [													// 测试点
    {
      "id": 0,													// 测试点 ID
      "result": "Waiting",										// 测试点结果
      "time": 0,												// 消耗时间
      "memory": 0,												// 占用内存
      "info": ""												// 编译信息、答案出错位置等
    },
    {
      "id": 1,
      "result": "Waiting",
      "time": 0,
      "memory": 0,
      "info": ""
    }
  ]
}
```

此后可以根据评测任务的 ID 来查询状态。若语言、用户、题目或比赛不存在，或不在比赛时间内，将得到 `ERR_NOT_FOUND` 错误；若用户或题目不在比赛中，将得到 `ERR_INVALID_ARGUMENT` 错误；若提交次数超出限制，将得到 `ERR_RATE_LIMIT ` 错误。

评测任务的 `state` 有下列可能：

| `state`    | 含义     |
| ---------- | -------- |
| `Queueing` | 正在排队 |
| `Running`  | 正在评测 |
| `Finished` | 评测完成 |
| `Canceled` | 评测取消 |

评测任务以及测试点的 `result` 有下列可能：

| `result`                | 含义         |
| ----------------------- | ------------ |
| `Waiting`               | 等待评测     |
| `Running`               | 正在运行     |
| `Accepted`              | 评测通过     |
| `Compilation Error`     | 编译错误     |
| `Compilation Success`   | 编译成功     |
| `Wrong Answer`          | 答案错误     |
| `Runtime Error`         | 运行时错误   |
| `Time Limit Exceeded`   | 超出时间限制 |
| `Memory Limit Exceeded` | 超出内存限制 |
| `System Error`          | OJ 系统错误  |

在鉴权模式下，若提交中的用户 ID 不是自己的 ID，将得到 `ERR_FORBIDDEN` 错误。

#### GET /jobs

**需求权限：**`User`

使用此 API 来查询筛选后的评测任务。参数在路径传递，格式如下：

```
GET /jobs?problem_id=0&state=Finished&arg=...&...
```

若不带参数，则查询所有评测任务。可出现的参数有：

| 参数         | 含义                       |
| ------------ | -------------------------- |
| `user_id`    | 用户 ID                    |
| `user_name`  | 用户名                     |
| `contest_id` | 比赛 ID                    |
| `problem_id` | 题目 ID                    |
| `language`   | 语言                       |
| `from`       | 任务的创建时间不早于此时间 |
| `to`         | 任务的创建时间不晚于此时间 |
| `state`      | 任务当前状态               |
| `result`     | 评测结果                   |

响应为一个数组，包含按任务创建时间升序排序的筛选后的评测任务。

#### GET /jobs/{id}

**需求权限：**`User`

使用此 API 查询 ID 为路径参数 `{id}` 的指定评测任务。

若任务不存在，将返回 `ERR_NOT_FOUND` 错误。

#### PUT /jobs/{id}

**需求权限：**`Author`

重新评测 ID 为路径参数 `{id}` 的指定评测任务。该任务将被修改为 `Queueing` 状态重新加入评测队列，返回结果与 `POST /jobs` 相同。

若任务不存在，将返回 `ERR_NOT_FOUND` 错误。若任务存在但状态不为 `Finished`，将返回 `ERR_INVALID_STATE` 错误。

#### DELETE /jobs/{id}

**需求权限：**`Author`

取消正在等待评测的 ID 为路径参数 `{id}` 的评测任务。该任务的状态将被修改为 `Canceled`。若无错误，响应体为空。

若任务不存在，将返回 `ERR_NOT_FOUND` 错误。若任务存在但状态不为 `Queueing`，将返回 `ERR_INVALID_STATE` 错误。

### 比赛

#### POST /contests

**需求权限：**`Author`

使用此 API 创建或更新比赛。请求格式如下：

```json
{
  "id": 1,								// 比赛 ID；若不指定，则创建新比赛
  "name": "Rust Course Project 2",		// 比赛名称
  "from": "2022-08-27T02:05:29.000Z",	// 比赛开始时间
  "to": "2022-08-27T02:05:30.000Z",		// 比赛结束时间
  "problem_ids": [						// 比赛包含题目
    2,
    1,
    3
  ],
  "user_ids": [							// 比赛包含用户
    5,
    4,
    6
  ],
  "submission_limit": 32				// 提交次数限制
}
```

若指定了 ID 但比赛不存在，或包含了不存在的题目或用户，则返回 `ERR_NOT_FOUND` 错误。

请求正确时，返回体除了一定会包含 `id` 字段外与请求体相同。

#### GET /contests

**需求权限：**`User`

使用此 API 获取比赛列表。返回按 ID 升序排列的数组，每个对象格式与 `POST contests/` 相同。

#### GET /contests/{id}

**需求权限：**`User`

获取指定 ID 的比赛。若比赛不存在，返回 `ERR_NOT_FOUND` 错误，否则返回格式与 `POST /contests` 相同。

#### GET /contests/{id}/ranklist

**需求权限：**`User`

获取指定 ID 的比赛的排行榜。按照所有题目的总分降序排名。请求格式如下：

```
GET /contests/{id}/ranklist?scoring_rule=...&tie_breaker=...
```

其中 `scoring_rule` 与 `tie_breaker` 为可选参数。`scoring_rule` 决定一名用户在一个题目上使用哪个提交来计算排名，`tie_breaker` 决定了总分相同时如何排名。二者可能的取值如下：

| `scoring_rule` | 含义                                 |
| -------------- | ------------------------------------ |
| `latest`       | 这是默认行为。使用最晚提交计算排名。 |
| `highest`      | 使用得分最高的提交计算排名。         |

| `tie_breaker`      | 含义                                                         |
| ------------------ | ------------------------------------------------------------ |
| `<DEFAULT>`        | 不指定时的默认行为，总分相同的用户排名也相同。               |
| `submission_time`  | 总分相同时，按用户所有用于计算排名的提交中最晚的提交升序计算排名。未提交任何题目的用户视为提交时间为无穷大。 |
| `submission_count` | 总分相同时，按用户在比赛中所有题目的提交数量总和升序计算排名。 |
| `user_id`          | 总分相同时，按用户 ID 升序计算排名。                         |

若比赛不存在，将返回 `ERR_NOT_FOUND` 错误，否则返回一个以排名为序的数组，其中每个对象的格式如下：

```json
{
    "user": {			// 用户信息
      "id": 0,
      "name": "root"
    },
    "rank": 1,			// 排名，从 1 开始
    "scores": [			// 在每个题目的得分，顺序与比赛配置中指定题目的顺序相同
      0,
      100
    ]
  }
```



由于其他评测技术方面的功能对于使用者来说是透明的，将在下一部分中一并叙述。

## 提高要求实现

### 用户鉴权 & 多角色支持

用户管理主要使用 [actix-jwt-auth-middleware](https://crates.io/crates/actix-jwt-auth-middleware) 库实现。在访问除了 `register` 与 `login` 外的 API 时，首先会通过存储在 Cookie 中的认证令牌来验证用户是否有权限访问 API，若未登录则返回 `401 Unauthorized`，若已登录但无权限则返回 `403 Forbidden`。Cookie 的生存期为 session，即用户关闭浏览器后，下次再访问时需要重新登录，在关闭浏览器前都无需重新登录。

用户可以通过注册获取一个权限等级为 `User` 的用户，该权限等级除了 `GET` API 外，只能访问更改密码以及提交评测两个 API。`Author` 权限的用户还可以重测题目、取消评测任务、添加或更新比赛。`Admin` 权限的用户还可以进行提权、新增或更新用户。0 号用户为 `root` 用户，其在数据库初始化时创建，拥有 `Admin` 权限。

除了 API 权限等级之外，提交题目时也会通过认证令牌来验证提交中声明的用户 ID 是否与实际登录的用户 ID 一致，即不允许代表其他用户进行提交。

用户的权限信息一并存储于数据库中，在持久化部分中一并叙述。

### 持久化存储

使用 `SQLite` 作为数据库后端进行持久化，数据库保存于 `oj.db` 文件中。使用 [diesel](https://crates.io/crates/diesel) ORM 来与数据库进行交互。使用 [diesel_migrations](https://crates.io/crates/diesel_migrations) 库来在程序启动时自动执行迁移。使用 [r2d2](https://crates.io/crates/r2d2) 库建立数据库连接池，每个服务器工作进程在需要时从连接池中获取与数据库的连接。同时，每个评测进程也持有对数据库的连接。

各数据表建立如下：

```sqlite
CREATE TABLE jobs (
    id INTEGER PRIMARY KEY NOT NULL,
    created_time DATETIME NOT NULL,
    updated_time DATETIME NOT NULL,
    source_code TEXT NOT NULL,
    lang TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    contest_id INTEGER NOT NULL,
    problem_id INTEGER NOT NULL,
    job_state INTEGER NOT NULL,
    result INTEGER NOT NULL,
    score DOUBLE NOT NULL,
    cases TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id),
    FOREIGN KEY(contest_id) REFERENCES contests(id)
)
```

其中 `job_state`、`result` 等枚举类型的值转换为整数类型存储，`cases` 转换为 `JSON` 格式存储。

```sqlite
CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    user_role INTEGER NOT NULL DEFAULT '0',
    user_name TEXT NOT NULL UNIQUE,
    passwd TEXT NOT NULL
);

INSERT INTO users (id, user_role, user_name, passwd) VALUES (0, 2, 'root', '#!/*<!--*#*SUPER_SECRET_PASSWORD*#*-->*/')
```

``` sqlite
CREATE TABLE contests (
    id INTEGER PRIMARY KEY NOT NULL,
    contest_name TEXT NOT NULL,
    contest_from DATETIME NOT NULL,
    contest_to DATETIME NOT NULL,
    problem_ids TEXT NOT NULL,
    user_ids TEXT NOT NULL,
    submission_limit INTEGER NOT NULL
)
```

其中 `problem_ids` 与 `user_ids` 两个数组类型的数据先转换为逗号分隔字符串后存储。

### 非阻塞评测 & 独立评测进程

在接收到评测请求后，OJ 会将其排入评测队列后立刻返回，不会阻塞等待。

使用 [amiquip](https://crates.io/crates/amiquip) 库调用 RabbitMQ 实现进程间通信及负载均衡。评测进程与服务端进程分离，在 OJ 启动时会创建一系列评测进程。在接收到评测请求后，服务端会将评测任务 ID 放入队列中，由 RabbitMQ 将消息分配给空闲的评测进程。评测进程收到消息后会进行评测并在评测过程中实时更新评测任务信息。

在取消评测任务时，服务端会将评测任务的状态修改为 `Canceled`。在开始评测前，评测进程会检查任务状态是否为 `Canceled`，若是则取消评测。

评测进程在成功完成评测后，需要 `ack` 消息，这意味着若评测进程由于某些因素退出，RabbitMQ 会将评测任务分配给其他评测进程继续执行。

## 完成作业感想

这一次作业相比于 Wordle 大作业，两个明显的变化是：技术含量提高了；基础部分所占的分数变少了。基础部分所占的分数变少意味着提高部分提供了丰富的选择，而技术含量的提高意味着实现难度的增大。

由于开始的几天比较摸鱼，我花了 3 天才实现完基础部分的功能。接下来在选择提高部分的功能时，我先看了一下各部分的难度比例：前端部分我比较擅长，但由于上个大作业已经选择了 GUI，我打算体验些不一样的东西。第四部分评测方式看上去相对容易实现，也没有任何新技术，仅仅是逻辑上多了一些处理。而用户管理与评测技术方面则是相对硬核而且需要学习较多新技术的。因此，我决定主要选择这两个方向的功能进行实现。

我选择从相对较熟悉的持久化存储开始实现。选择从数据库开始还有一个原因是实现后对其他技术的实现也能提供便利。然而，我选择了一条“错误”的道路——使用 Diesel ORM（根据后来与其他同学的交谈，直接用 rusqlite 裸写 SQL 语句的体验要好得多）。根据我的了解，Rust 目前相对成熟可用的 ORM 有 Diesel 和 SeaORM，其中 Diesel 的年代更加久远一些，于是我选择尝试 Diesel。但我花了一天了解 Diesel 的使用方式后，真正上手实现却发现了许多各种各样的问题：由于一开始实现时没有注意与数据库的兼容性，很多类型的设计都不太适合直接作为 Diesel 的模型使用，而需要重新定义一个新的类型，并在用于 API 的类型与用于数据库的类型之间来回转换。同时，SQLite 作为一个相当轻量的数据库在带来了便利的同时也带来了不少麻烦：它不能保存枚举类型，因此对每个枚举类型都要定义一个从 SQL 转换和转换至 SQL 的函数，而这些函数全是大段的 boilerplate，写起来非常繁琐；它也不支持数组类型，因此我不得不将数组转换为逗号分隔字符串，作为文本类型存入数据库。对于像评测任务的测试点结果，我更是摆烂般地直接将整个数组转为 JSON 存入数据库（虽然要为这一行为开脱也是有理由的：不像其他数据，测试点结果不会被用来筛选，因此整个作为 JSON 保存也无伤大雅）。另外一个严重的问题是，Diesel 的编译信息非常非常地不友好，基本上没办法从编译错误里看出真正的错误原因（这令人想到 C++20 concepts 出现以前的模板编译错误），这导致我花了很长时间研究到底哪里写错了，体验甚至不如裸写 SQL，更别提 Django 那写起来令人心旷神怡的 ORM 了。

写完数据库后，我顺手把比赛支持写了，这时才体验到没有新技术的功能写起来有多么愉快。但我并不会因此放弃学习新技术，于是下一步我决定实现非阻塞评测。这部分的第一个评测点基本上是送的，因为只要加一句 `web::block` 就结束了；第二部分其实也还比较容易实现，但是因为现在数据库操作是由评测函数来进行，这带来了一些新的问题：我遇到了访问数据库时报 Database is Locked 的错误。这时我才知道 SQLite 不能同时有多个线程同时写入数据库，否则就会直接报错，而在 MySQL 等数据库这个操作一般会将操作排队进行。好在通过万能的 StackOverflow，我找到了一个方便的解决方案：让 SQLite 在数据库锁定时稍等一段时间再尝试。

在写非阻塞评测的过程中，由于在评测函数里有很多重复的代码，但它们又无法封装到函数里（例如，这些操作的结尾需要 `continue`），重复写这些代码让我感到非常不快。因此我学习了一下早有耳闻其强大的 Rust 宏，将重复操作提取到宏中，重写了评测函数。因此，在非阻塞评测这个 commit 里，我的代码行数甚至减少了 85 行。在写 Rust 宏的过程中，我深感这个功能既有充分的自由性、强大的功能性，同时又有充分的编译时检查，比 C 的宏体验要好上太多，也比 Java 这种处处都是 boilerplate 的语言写起来要爽得多。

下一个内容是独立评测进程。为了实现这个功能，我首先学习了一下 RabbitMQ，发现这是个相当好用的东西。在 OJ 中实现这个功能倒没有遇上太多困难，因为我所做的跨进程通信较为简单，仅仅是服务器将评测任务 ID 放入消息队列，然后评测进程从中取出 ID 进行评测。但是这个功能却带来了一个在这次大作业中，花费我时间最多、也带给我最深心理阴影的 bug。

这个 bug 的起因比较离奇，但它的表现更为离奇。在我本机测试时，一切正常，push 到 GitLab 上后，CI 却失败了。查看日志发现是在进行数据库操作时，发生了 Disk I/O Error 错误。我四处搜索并没有得到关于这一错误的更多消息，于是将 CI 使用的镜像拉取下来打算在本地查找错误根源。但是更离奇的事情发生了：在我本地用这个镜像运行，结果完全正确，只有在 CI 上才会发生错误。如此一来，我根本无法复现错误。我向助教求助，助教只告诉我在他的环境下跑出了一样的错误，这让我更加不知所措。也就是说，在我能接触到的环境，我都无法复现错误，然而我无法接触到的环境都出现了错误。

在进行一天无谓的挣扎之后，我终于决定直接在 GitLab 上开始调试：新建了一个 debug 分支，直接每次提交后看 CI 的日志。这样尝试了一系列我也不知道能不能解决问题的操作之后，终于有一次 CI 通过了，我当时就高兴坏了。那一次所做的修改是：为每个测试的 OJ 进程开辟一个不同的消息队列。

这个错误为何会出现呢？其实要说的话，根本原因是测试框架太过于暴力。正常来说，我的 OJ 进程结束时将会通知评测进程一并结束。但是，测试框架在一个点测试通过之后会直接 kill 掉 OJ 进程（我不理解为何不使用那个定义的 /internal/exit 接口），导致此时根本无法通知评测进程结束。于是，这些评测进程就会开始抢下一个测试点的评测任务。但是，上一个测试点的评测进程连接的数据库此时已经被删除重建，因此他们访问数据库会报错退出。这是在我本机测试没有问题的原因。但运行 CI 的服务器上有较多 CPU 核心（似乎是 24 个），这导致 二十多个进程短时间内接连尝试锁定数据库文件进行访问，我猜测可能触发了 SQLite 对多进程同时访问的某种限制导致了 Disk I/O Error。尽管我也无法肯定这就是问题的确切原因，但是将每个测试点的消息队列独立出来确实解决了问题，而这距离发现这个 bug 已经过去了两天。

到这个时间点，可用于开发的时间大概仅剩 2 天。我稍微扫了一眼评测安全性的内容，发现这部分内容基本都需要通过 unsafe Rust 直接调用 libc 来实现，并没有什么现成的好用的库。于是我决定实现用户鉴权的功能。这部分内容我找到了一个还算好用的轮子 [actix-jwt-auth-middleware](https://crates.io/crates/actix-jwt-auth-middleware)，然而从它寥寥的下载量可以看出，它是一个相当不完善的库。例如，它基本没有扩展性，只有它预定义的一两个功能，而且一旦想用它，它就会强制进行鉴权，没有任何接口允许关闭鉴权。然而为了自动测试的需要，又需要能够关闭鉴权。后来我选择将鉴权功能单独提取为一个 feature，只有开启时才会启用鉴权。顺带一提，这时我才知道 `#[cfg]` 宏居然可以单独作用于某一个函数参数，这让 feature 的提取简单了许多（否则，我就得为每个函数一模一样地实现两次了）。

在完成本次大作业的过程中，我学到了非常多的新知识，例如用 Rust 操作数据库、使用消息队列、使用 JWT 进行鉴权等。我深感 Rust 严格的编译期检查事实上省去了很多运行时调试的过程，提升了开发效率。尽管 Rust 课程已经结束，但我还将会继续探索 Rust 的可能性：它让我在一众近乎换皮的编程语言中看到了未来编程语言可能的发展方向。
