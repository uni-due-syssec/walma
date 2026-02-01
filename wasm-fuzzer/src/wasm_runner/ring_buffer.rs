use std::collections::VecDeque;

pub struct RingBuffer<T> {
    max_size: usize,
    stack: VecDeque<T>
}

impl<T> RingBuffer<T> {
    pub fn new(max_size: usize) -> Self {
        return Self {
            max_size,
            stack: VecDeque::with_capacity(max_size),
        };
    }

    pub fn push(&mut self, item: T) {
        if self.stack.len() == self.max_size {
            self.stack.pop_front();
        }

        self.stack.push_back(item);
    }

    #[allow(dead_code)]
    pub fn pop(&mut self) -> Option<T> {
        return self.stack.pop_back();
    }

    #[allow(dead_code)]
    pub fn is_full(&self) -> bool {
        return self.stack.len() == self.max_size;
    }

    pub fn get_stack(&self) -> &VecDeque<T> {
        return &self.stack;
    }

    pub fn get_max_size(&self) -> usize {
        return self.max_size;
    }
    
    #[allow(dead_code)]
    pub fn peek(&self) -> Option<&T> {
        return self.stack.back();
    }
}