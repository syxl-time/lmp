// Copyright 2024 The LMP Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://github.com/linuxkerneltravel/lmp/blob/develop/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// author: luiyanbing@foxmail.com
//
// 包装用于采集调用栈数据的eBPF程序，声明接口、通用成员和一些辅助结构

#ifndef _SA_STACK_COLLECTOR_H__
#define _SA_STACK_COLLECTOR_H__

#include <stdint.h>
#include <unistd.h>
#include <vector>
#include <string>
#include "sa_user.h"

struct Scale
{
    const char *Type, *Unit;
    int64_t Period;
};

/// @brief count类，主要是为了重载比较运算，便于自动排序
struct CountItem
{
    psid k;
    double v;
    CountItem(psid k, double v) : k(k), v(v){};

    /// @brief count对象的大小取决于val的大小
    /// @param b 要比较的对象
    /// @return 小于b则为真，否则为假
    friend bool operator<(const CountItem a, const CountItem b);
};

class StackCollector
{
protected:
    int self_pid = -1;
    struct bpf_object *obj = NULL;

    // 默认显示计数的变化情况，即每次输出数据后清除计数
    bool showDelta = true;

public:
    Scale scale = {0};

    int pid = -1; // 用于设置ebpf程序跟踪的pid
    int err = 0;  // 用于保存错误代码

    bool ustack = false; // 是否跟踪用户栈
    bool kstack = false; // 是否跟踪内核栈

protected:
    std::vector<CountItem> *sortedCountList(void);

    /// @brief 将缓冲区的数据解析为特定值
    /// @param  无
    /// @return 解析出的值
    virtual double count_value(void *data) = 0;

public:
    StackCollector();
    operator std::string();

    /// @brief 负责ebpf程序的加载、参数设置和打开操作
    /// @param  无
    /// @return 成功则返回0，否则返回负数
    virtual int load(void) = 0;

    /// @brief 将ebpf程序挂载到跟踪点上
    /// @param  无
    /// @return 成功则返回0，否则返回负数
    virtual int attach(void) = 0;

    /// @brief 断开ebpf的跟踪点和处理函数间的连接
    /// @param  无
    virtual void detach(void) = 0;

    /// @brief 卸载ebpf程序
    /// @param  无
    virtual void unload(void) = 0;

// 声明eBPF骨架
#define declareEBPF(func) struct func##_bpf *skel = NULL;

/// @brief 加载、初始化参数并打开指定类型的ebpf程序
/// @param ... 一些ebpf程序全局变量初始化语句
/// @note 失败会使上层函数返回-1
#define StackProgLoadOpen(...)                         \
    {                                                  \
        skel = skel->open(NULL);                       \
        CHECK_ERR(!skel, "Fail to open BPF skeleton"); \
        __VA_ARGS__;                                   \
        skel->rodata->trace_user = ustack;             \
        skel->rodata->trace_kernel = kstack;           \
        skel->rodata->self_pid = self_pid;             \
        err = skel->load(skel);                        \
        CHECK_ERR(err, "Fail to load BPF skeleton");   \
        obj = skel->obj;                               \
    }

#define defaultAttach                                    \
    {                                                    \
        err = skel->attach(skel);                        \
        CHECK_ERR(err, "Failed to attach BPF skeleton"); \
    }

#define defaultDetach           \
    {                           \
        if (skel)               \
        {                       \
            skel->detach(skel); \
        }                       \
    }

#define defaultUnload            \
    {                            \
        if (skel)                \
        {                        \
            skel->destroy(skel); \
        }                        \
        skel = NULL;             \
    }
};

#endif